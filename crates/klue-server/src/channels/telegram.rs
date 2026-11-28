//! The Telegram bot: alerts and the daily report, and a menu to look at Home, a carrier's
//! lines, a line in full with its graphs, or the whole report as a PDF.
//!
//! Only chats linked from the app (a one-time link from Settings → Telegram) get answers.
//! It polls Telegram for messages, so the server needs no public web address of its own.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use klue_core::{
    Carrier,
    alerts::AlertEvent,
    store::{Account, Card, Group, Lang},
};
use klue_engine::remote::{TelegramChat, TelegramLink, secret};
use klue_report::{Look, SITE, words};
use serde_json::{Value, json};
use tokio::sync::Notify;
use wreq::multipart::{Form, Part};

use super::text::{self, LineFacts};
use crate::state::State;

/// How long a link to add a chat works.
const LINK_FOR: chrono::Duration = chrono::Duration::minutes(15);

#[derive(Default)]
pub struct Telegram {
    /// Wakes the bot when its token or settings change.
    pub restart: Notify,
    /// One-time codes from Settings → Telegram, until they expire.
    codes: Mutex<HashMap<String, DateTime<Utc>>>,
    /// Captcha photos sent, by chat and message, to the captcha they ask about.
    captchas: Mutex<HashMap<(i64, i64), String>>,
}

/// Telegram's Bot API for one token. `KLUE_TELEGRAM_API` points it elsewhere, for tests.
struct Api {
    client: wreq::Client,
    base: String,
}

impl Api {
    fn new(token: &str) -> Result<Self, String> {
        let root = std::env::var("KLUE_TELEGRAM_API").unwrap_or_else(|_| "https://api.telegram.org".into());
        let client = klue_core::web_client(Duration::from_secs(75)).map_err(|e| e.to_string())?;
        Ok(Self { client, base: format!("{}/bot{token}", root.trim_end_matches('/')) })
    }

    async fn call(&self, method: &str, body: Value) -> Result<Value, String> {
        let resp = self.client.post(format!("{}/{method}", self.base)).json(&body).send().await.map_err(|e| e.to_string())?;
        read(resp).await
    }

    async fn upload(&self, method: &str, form: Form) -> Result<Value, String> {
        let resp = self.client.post(format!("{}/{method}", self.base)).multipart(form).send().await.map_err(|e| e.to_string())?;
        read(resp).await
    }
}

async fn read(resp: wreq::Response) -> Result<Value, String> {
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    if v["ok"].as_bool() == Some(true) {
        Ok(v["result"].clone())
    } else {
        Err(v["description"].as_str().unwrap_or("Telegram refused the request").to_string())
    }
}

fn api(state: &State) -> Option<Api> {
    Api::new(&state.vault.get(secret::TELEGRAM_TOKEN)?).ok()
}

fn chats(state: &State) -> Vec<TelegramChat> {
    state.settings().telegram.chats
}

/// Runs the bot while there's a token, and starts over when it changes.
pub async fn run(state: Arc<State>) {
    loop {
        let Some(api) = api(&state) else {
            state.telegram.restart.notified().await;
            continue;
        };
        tokio::select! {
            _ = state.telegram.restart.notified() => {}
            result = poll(&state, &api) => {
                if let Err(e) = result {
                    state.channel_result("Telegram", Err(e));
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    }
}

async fn poll(state: &Arc<State>, api: &Api) -> Result<(), String> {
    let me = api.call("getMe", json!({})).await?;
    let bot = me["username"].as_str().map(String::from);
    state.update_settings(|s| s.telegram.bot = bot.clone());
    set_commands(api).await;
    state.channel_result("Telegram", Ok(()));
    let mut offset: i64 = 0;
    loop {
        let updates = api.call("getUpdates", json!({ "offset": offset, "timeout": 50, "allowed_updates": ["message", "callback_query"] })).await?;
        for update in updates.as_array().into_iter().flatten() {
            offset = offset.max(update["update_id"].as_i64().unwrap_or(0) + 1);
            if let Err(e) = handle(state, api, update).await {
                tracing::warn!("Telegram: {e}");
            }
        }
    }
}

async fn set_commands(api: &Api) {
    for lang in [Lang::En, Lang::Ar] {
        let commands = json!([
            { "command": "home", "description": words::pick(lang, "Your Home lines at a glance", "خطوطك في الرئيسية في لمحة") },
            { "command": "report", "description": words::pick(lang, "The full report, as a PDF", "التقرير الكامل بصيغة PDF") },
            { "command": "refresh", "description": words::pick(lang, "Check every line now", "تحديث كل الخطوط الآن") },
            { "command": "help", "description": words::pick(lang, "What this bot does", "ماذا يفعل هذا البوت") },
        ]);
        let mut body = json!({ "commands": commands });
        if lang == Lang::Ar {
            body["language_code"] = json!("ar");
        }
        let _ = api.call("setMyCommands", body).await;
    }
}

/// A link that links a chat when opened in Telegram, for Settings → Telegram.
pub fn link(state: &State) -> Option<TelegramLink> {
    let bot = state.settings().telegram.bot?;
    let code: String = (0..12).map(|_| char::from(b"abcdefghijkmnpqrstuvwxyz23456789"[rand::random_range(0..32)])).collect();
    let expires = Utc::now() + LINK_FOR;
    if let Ok(mut codes) = state.telegram.codes.lock() {
        codes.retain(|_, until| *until > Utc::now());
        codes.insert(code.clone(), expires);
    }
    Some(TelegramLink { url: format!("https://t.me/{bot}?start={code}"), expires })
}

fn html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// "Sent by Klue · Home VPS", in italics, "Klue" linked.
fn footer(look: &Look) -> String {
    let mut s = format!("<i>{} <a href=\"{SITE}\">Klue</a>", text::sent_by(look.lang));
    if let Some(server) = &look.server {
        s.push_str(&format!(" · {}", html(server)));
    }
    s + "</i>"
}

/// The keyboard under the chat: Home, the report, and each carrier with lines.
fn menu(state: &State, lang: Lang) -> Value {
    let data = state.engine.snapshot().data;
    let carriers: Vec<Value> = data.carriers().into_iter().filter(|c| data.accounts_for(*c).next().is_some()).map(|c| json!({ "text": words::carrier_name(c, lang) })).collect();
    let mut rows = vec![vec![json!({ "text": home_button(lang) }), json!({ "text": report_button(lang) })]];
    for chunk in carriers.chunks(4) {
        rows.push(chunk.to_vec());
    }
    json!({ "keyboard": rows, "resize_keyboard": true, "is_persistent": true })
}

fn home_button(lang: Lang) -> &'static str {
    words::pick(lang, "🏠 Home", "🏠 الرئيسية")
}

fn report_button(lang: Lang) -> &'static str {
    words::pick(lang, "📄 Full report", "📄 التقرير الكامل")
}

async fn handle(state: &Arc<State>, api: &Api, update: &Value) -> Result<(), String> {
    if let Some(query) = update.get("callback_query") {
        let chat = query["message"]["chat"]["id"].as_i64().unwrap_or(0);
        let _ = api.call("answerCallbackQuery", json!({ "callback_query_id": query["id"] })).await;
        if !linked(state, chat) {
            return Ok(());
        }
        return pressed(state, api, chat, query["data"].as_str().unwrap_or_default()).await;
    }
    let message = &update["message"];
    let chat = message["chat"]["id"].as_i64().unwrap_or(0);
    let text = message["text"].as_str().unwrap_or_default().trim().to_string();
    let look = super::look(state, None);
    let lang = look.lang;

    if let Some(code) = text.strip_prefix("/start") {
        let code = code.trim();
        if !linked(state, chat) {
            let valid = state.telegram.codes.lock().ok().and_then(|mut c| c.remove(code)).is_some_and(|until| until > Utc::now());
            if !valid {
                let private = words::pick(lang, "This bot is private. Link this chat from Klue → Settings → Telegram.", "هذا البوت خاص. اربط هذه المحادثة من Klue ← الإعدادات ← تيليجرام.");
                return api.call("sendMessage", json!({ "chat_id": chat, "text": private })).await.map(|_| ());
            }
            let name = chat_name(&message["chat"]);
            state.update_settings(|s| s.telegram.chats.push(TelegramChat { id: chat, name }));
        }
        let welcome = if lang == Lang::Ar {
            format!("✅ <b>تم ربط هذه المحادثة بـ Klue</b>\n\nستصلك هنا التنبيهات كما ضبطتها، ويمكنك رؤية خطوطك في أي وقت من الأزرار بالأسفل.\n\n{}", footer(&look))
        } else {
            format!("✅ <b>This chat is linked to Klue</b>\n\nAlerts arrive here as you set them up, and the buttons below show your lines whenever you like.\n\n{}", footer(&look))
        };
        return api.call("sendMessage", json!({ "chat_id": chat, "text": welcome, "parse_mode": "HTML", "reply_markup": menu(state, lang), "disable_web_page_preview": true })).await.map(|_| ());
    }
    if !linked(state, chat) {
        return Ok(());
    }
    // A reply to a captcha photo is its answer.
    if let Some(replied) = message["reply_to_message"]["message_id"].as_i64() {
        let id = state.telegram.captchas.lock().ok().and_then(|mut c| c.remove(&(chat, replied)));
        if let Some(id) = id {
            crate::state::answer_captcha(state, &id, Some(text.clone()));
            let thanks = words::pick(lang, "Thanks, trying it now.", "شكراً، جارٍ تجربته الآن.");
            return api.call("sendMessage", json!({ "chat_id": chat, "text": thanks })).await.map(|_| ());
        }
    }
    let command = text.split_whitespace().next().unwrap_or_default().split('@').next().unwrap_or_default();
    let is = |en: &str, ar: &str| text == en || text == ar;
    if command == "/home" || is(home_button(Lang::En), home_button(Lang::Ar)) {
        send_home(state, api, chat, None).await
    } else if command == "/report" || is(report_button(Lang::En), report_button(Lang::Ar)) {
        send_report(state, api, chat).await
    } else if command == "/refresh" {
        let s = state.clone();
        tokio::spawn(async move { s.engine.refresh_all().await });
        let checking = words::pick(lang, "🔄 Checking every line. The numbers here update when it's done.", "🔄 جارٍ تحديث كل الخطوط. تتحدث الأرقام هنا عند الانتهاء.");
        api.call("sendMessage", json!({ "chat_id": chat, "text": checking })).await.map(|_| ())
    } else if let Some(carrier) = Carrier::ALL.into_iter().find(|c| is(words::carrier_name(*c, Lang::En), words::carrier_name(*c, Lang::Ar))) {
        send_carrier(state, api, chat, carrier).await
    } else {
        let help = if lang == Lang::Ar {
            format!("<b>Klue</b> يتابع خطوطك ويرسل التنبيهات هنا.\n\n🏠 <b>الرئيسية</b>: خطوطك في لمحة\n📄 <b>التقرير الكامل</b>: كل الخطوط ورسومها في ملف PDF\nوأزرار الشركات لرؤية خط بعينه.\n\n{}", footer(&look))
        } else {
            format!("<b>Klue</b> keeps an eye on your lines and sends alerts here.\n\n🏠 <b>Home</b>: your lines at a glance\n📄 <b>Full report</b>: every line and its graphs, as a PDF\nand a button per carrier to see one line in full.\n\n{}", footer(&look))
        };
        api.call("sendMessage", json!({ "chat_id": chat, "text": help, "parse_mode": "HTML", "reply_markup": menu(state, lang), "disable_web_page_preview": true })).await.map(|_| ())
    }
}

fn linked(state: &State, chat: i64) -> bool {
    chats(state).iter().any(|c| c.id == chat)
}

fn chat_name(chat: &Value) -> String {
    chat["title"]
        .as_str()
        .map(String::from)
        .or_else(|| {
            let first = chat["first_name"].as_str()?;
            Some(match chat["last_name"].as_str() {
                Some(last) => format!("{first} {last}"),
                None => first.to_string(),
            })
        })
        .or_else(|| chat["username"].as_str().map(|u| format!("@{u}")))
        .unwrap_or_else(|| "Telegram".into())
}

async fn pressed(state: &Arc<State>, api: &Api, chat: i64, data: &str) -> Result<(), String> {
    let (kind, id) = data.split_once(':').unwrap_or((data, ""));
    match kind {
        "line" => send_line(state, api, chat, id).await,
        "graph" => send_graph(state, api, chat, id).await,
        "tab" => send_home(state, api, chat, Some(id)).await,
        "carrier" => match Carrier::ALL.into_iter().find(|c| c.id() == id) {
            Some(c) => send_carrier(state, api, chat, c).await,
            None => Ok(()),
        },
        "check" => {
            let lang = super::look(state, None).lang;
            let checking = words::pick(lang, "🔄 Checking…", "🔄 جارٍ التحديث…");
            let _ = api.call("sendMessage", json!({ "chat_id": chat, "text": checking })).await;
            state.engine.refresh(id, true).await;
            send_line(state, api, chat, id).await
        }
        _ => Ok(()),
    }
}

/// A photo with its caption and buttons.
async fn photo(api: &Api, chat: i64, png: Vec<u8>, caption: String, buttons: Option<Value>) -> Result<(), String> {
    let mut form = Form::new()
        .text("chat_id", chat.to_string())
        .text("caption", caption)
        .text("parse_mode", "HTML")
        .part("photo", Part::bytes(png).file_name("klue.png").mime_str("image/png").map_err(|e| e.to_string())?);
    if let Some(buttons) = buttons {
        form = form.text("reply_markup", buttons.to_string());
    }
    api.upload("sendPhoto", form).await.map(|_| ())
}

/// A Home tab as a picture of its cards, with a line per card and the other tabs as buttons.
async fn send_home(state: &Arc<State>, api: &Api, chat: i64, tab: Option<&str>) -> Result<(), String> {
    let look = super::look(state, None);
    let lang = look.lang;
    let data = state.engine.snapshot().data;
    let groups = data.groups();
    let group: Group = tab.and_then(|t| data.group(t)).unwrap_or_else(|| groups[0].clone());
    let index = groups.iter().position(|g| g.id == group.id).unwrap_or(0);
    let mut cards: Vec<(Card, Account)> = data.group_cards(&group.id).into_iter().map(|(c, a)| (c.clone(), a.clone())).collect();
    // A Home that hasn't been set up shows every line.
    if cards.is_empty() && data.groups.iter().all(|g| g.cards.is_empty()) {
        cards = data.accounts.iter().filter(|a| !a.archived).map(|a| (Card::main(&a.id), a.clone())).collect();
    }
    let title = group.name.clone().unwrap_or_else(|| if index == 0 { words::pick(lang, "My lines", "خطوطي").to_string() } else { format!("{} {}", words::pick(lang, "Group", "مجموعة"), index + 1) });
    let pairs: Vec<(&Card, &Account)> = cards.iter().map(|(c, a)| (c, a)).collect();
    let look2 = look.clone();
    let title2 = title.clone();
    let owned = cards.clone();
    let png = tokio::task::spawn_blocking(move || {
        let pairs: Vec<(&Card, &Account)> = owned.iter().map(|(c, a)| (c, a)).collect();
        klue_report::home_png(&title2, &pairs, &look2)
    })
    .await
    .ok()
    .flatten()
    .ok_or("couldn't draw Home")?;
    let mut caption = format!("🏠 <b>{}</b>\n", html(&title));
    for (card, account) in &pairs {
        let f = text::facts(account, &look);
        let bundle = card.bundle.as_deref().and_then(|k| account.usage.as_ref()?.bundles.iter().find(|b| b.key == k));
        let (left, pct, low) = match (bundle, &f.main) {
            (Some(b), _) => (text::amount(b.remaining, b.unit, lang), b.remaining_pct(), account.alert_for(b, false).is_triggered(b)),
            (None, Some(m)) => (m.left.clone(), m.pct, m.low),
            (None, None) => ("—".into(), 0.0, false),
        };
        let mark = if low { " ⚠️" } else { "" };
        caption.push_str(&format!("\n• <b>{}</b> · {} ({:.0}%){mark}", html(&f.name), html(&left), pct));
    }
    caption.push_str(&format!("\n\n{}", footer(&look)));
    let others: Vec<Value> = groups.iter().enumerate().filter(|(_, g)| g.id != group.id).map(|(i, g)| json!({ "text": g.name.clone().unwrap_or_else(|| if i == 0 { words::pick(lang, "My lines", "خطوطي").to_string() } else { format!("{} {}", words::pick(lang, "Group", "مجموعة"), i + 1) }), "callback_data": format!("tab:{}", g.id) })).collect();
    let buttons = (!others.is_empty()).then(|| json!({ "inline_keyboard": others.chunks(2).map(<[Value]>::to_vec).collect::<Vec<_>>() }));
    photo(api, chat, png, truncate(caption), buttons).await
}

/// Which of a carrier's lines to look at.
async fn send_carrier(state: &Arc<State>, api: &Api, chat: i64, carrier: Carrier) -> Result<(), String> {
    let look = super::look(state, None);
    let lang = look.lang;
    let data = state.engine.snapshot().data;
    let lines: Vec<&Account> = data.accounts_for(carrier).collect();
    let name = words::carrier_name(carrier, lang);
    if lines.is_empty() {
        let none = if lang == Lang::Ar { format!("لا توجد خطوط {name} في Klue.") } else { format!("There are no {name} lines in Klue.") };
        return api.call("sendMessage", json!({ "chat_id": chat, "text": none })).await.map(|_| ());
    }
    let buttons: Vec<Vec<Value>> = lines
        .iter()
        .map(|a| {
            let f = text::facts(a, &look);
            let left = f.main.map(|m| format!(" · {}", m.left)).unwrap_or_default();
            vec![json!({ "text": format!("{}{left}", f.name), "callback_data": format!("line:{}", a.id) })]
        })
        .collect();
    let ask = if lang == Lang::Ar { format!("<b>خطوط {name}</b>\nأي خط تريد رؤيته؟") } else { format!("<b>{name} lines</b>\nWhich one would you like to see?") };
    api.call("sendMessage", json!({ "chat_id": chat, "text": ask, "parse_mode": "HTML", "reply_markup": { "inline_keyboard": buttons } })).await.map(|_| ())
}

/// A line in full: a picture of it, the facts, and buttons for its graphs and a check.
async fn send_line(state: &Arc<State>, api: &Api, chat: i64, id: &str) -> Result<(), String> {
    let look = super::look(state, None);
    let Some(account) = state.engine.snapshot().data.account(id).cloned() else { return Ok(()) };
    let (a, l) = (account.clone(), look.clone());
    let png = tokio::task::spawn_blocking(move || klue_report::line_png(&a, &l)).await.ok().flatten().ok_or("couldn't draw the line")?;
    let caption = line_caption(&text::facts(&account, &look), &look);
    let lang = look.lang;
    let buttons = json!({ "inline_keyboard": [
        [
            { "text": words::pick(lang, "📈 Usage graph", "📈 رسم الاستخدام"), "callback_data": format!("graph:{id}") },
            { "text": words::pick(lang, "🔄 Check now", "🔄 تحديث الآن"), "callback_data": format!("check:{id}") },
        ],
        [{ "text": format!("{} {}", words::pick(lang, "⬅ All", "➡ كل خطوط"), words::carrier_name(account.carrier, lang)), "callback_data": format!("carrier:{}", account.carrier.id()) }],
    ]});
    photo(api, chat, png, caption, Some(buttons)).await
}

/// A line's facts, for under its picture or on their own.
pub fn line_caption(f: &LineFacts, look: &Look) -> String {
    let lang = look.lang;
    let ar = lang == Lang::Ar;
    let mut s = format!("<b>{}</b> · {}\n<code>{}</code>", html(&f.name), f.carrier_name, html(&f.number));
    if let Some(m) = &f.main {
        let mark = if m.low { " ⚠️" } else { "" };
        let of = if ar { format!("من {}", m.total) } else { format!("of {}", m.total) };
        s.push_str(&format!("\n\n<b>{}</b> {} {of} ({:.0}%){mark}\n{}", html(&m.left), words::pick(lang, "left", "متبقية"), m.pct, text::blocks(m.pct)));
        if let Some(package) = &f.package {
            s.push_str(&format!("\n{}", html(package)));
        }
    }
    if let Some((date, when)) = &f.renews {
        s.push_str(&format!("\n{} {} · {}", words::pick(lang, "Renews", "يتجدد"), html(date), when));
    }
    if let Some(balance) = &f.balance {
        s.push_str(&format!("\n{} {}", words::pick(lang, "Balance", "الرصيد"), balance));
    }
    if !f.others.is_empty() {
        s.push_str(&format!("\n\n<b>{}</b>", words::pick(lang, "Other bundles", "باقات أخرى")));
        for b in &f.others {
            let of = if ar { format!("من {}", b.total) } else { format!("of {}", b.total) };
            s.push_str(&format!("\n• {}: <b>{}</b> {of}", html(&b.name), html(&b.left)));
        }
    }
    let mut tail = Vec::new();
    if f.failing {
        tail.push(words::pick(lang, "⚠️ The last check failed", "⚠️ فشل آخر تحديث").to_string());
    }
    if let Some(checked) = &f.checked {
        tail.push(checked.clone());
    }
    if !tail.is_empty() {
        s.push_str(&format!("\n\n{}", tail.join(" · ")));
    }
    s.push_str(&format!("\n{}", footer(look)));
    truncate(s)
}

/// Captions stop at 1024 characters.
fn truncate(s: String) -> String {
    if s.chars().count() <= 1024 {
        return s;
    }
    // Cut whole lines, keeping the footer.
    let footer_start = s.rfind("\n<i>").unwrap_or(s.len());
    let (body, footer) = s.split_at(footer_start);
    let mut out = String::new();
    for line in body.lines() {
        if out.chars().count() + line.chars().count() + footer.chars().count() + 3 > 1024 {
            out.push_str("\n…");
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    out + footer
}

async fn send_graph(state: &Arc<State>, api: &Api, chat: i64, id: &str) -> Result<(), String> {
    let look = super::look(state, None);
    let Some(account) = state.engine.snapshot().data.account(id).cloned() else { return Ok(()) };
    let (a, l) = (account.clone(), look.clone());
    let Some(png) = tokio::task::spawn_blocking(move || klue_report::usage_png(&a, None, &l)).await.ok().flatten() else {
        let none = words::pick(look.lang, "There's nothing to draw yet.", "لا يوجد ما يُرسم بعد.");
        return api.call("sendMessage", json!({ "chat_id": chat, "text": none })).await.map(|_| ());
    };
    let caption = format!("📈 <b>{}</b> · {}\n{}", html(&account.display_name()), words::pick(look.lang, "what was left and what was used", "المتبقي والمستخدم"), footer(&look));
    photo(api, chat, png, caption, None).await
}

async fn send_report(state: &Arc<State>, api: &Api, chat: i64) -> Result<(), String> {
    let _ = api.call("sendChatAction", json!({ "chat_id": chat, "action": "upload_document" })).await;
    let pdf = super::report_pdf(state, None, None).await.ok_or("couldn't make the report")?;
    let look = super::look(state, None);
    document(api, chat, pdf, &text::report_file(&look), format!("📄 <b>{}</b>\n{}", html(&text::report_title(&look, false)), footer(&look))).await
}

async fn document(api: &Api, chat: i64, pdf: Vec<u8>, file: &str, caption: String) -> Result<(), String> {
    let form = Form::new()
        .text("chat_id", chat.to_string())
        .text("caption", truncate(caption))
        .text("parse_mode", "HTML")
        .part("document", Part::bytes(pdf).file_name(file.to_string()).mime_str("application/pdf").map_err(|e| e.to_string())?);
    api.upload("sendDocument", form).await.map(|_| ())
}

/// An alert to every linked chat, with a button to open the line.
pub async fn alert(state: &Arc<State>, account: &Account, event: &AlertEvent) -> Result<(), String> {
    let Some(api) = api(state) else { return Ok(()) };
    let look = super::look(state, None);
    let a = text::alert(account, event, &look);
    let icon = match a.level {
        text::Level::Warning => "⚠️",
        text::Level::Reminder => "🗓",
    };
    let mut s = format!("{icon} <b>{}</b>\n{}", html(&a.title), html(&a.headline));
    if let Some(pct) = a.pct {
        s.push_str(&format!("\n{} {:.0}%", text::blocks(pct), pct));
    }
    if let Some(detail) = &a.detail {
        s.push_str(&format!("\n{}", html(detail)));
    }
    s.push_str(&format!("\n\n{}", footer(&look)));
    let button = json!({ "inline_keyboard": [[{ "text": words::pick(look.lang, "Open the line", "فتح الخط"), "callback_data": format!("line:{}", account.id) }]] });
    for chat in chats(state) {
        api.call("sendMessage", json!({ "chat_id": chat.id, "text": s, "parse_mode": "HTML", "reply_markup": button, "disable_web_page_preview": true })).await?;
    }
    Ok(())
}

/// The daily report to every linked chat: the PDF, with each line's figure in the caption.
pub async fn daily(state: &Arc<State>, pdf: &[u8], lines: &[Account]) -> Result<(), String> {
    let Some(api) = api(state) else { return Ok(()) };
    let look = super::look(state, None);
    let mut caption = format!("📄 <b>{}</b>\n", html(&text::report_title(&look, true)));
    for account in lines {
        let f = text::facts(account, &look);
        if let Some(m) = &f.main {
            let mark = if m.low { " ⚠️" } else { "" };
            caption.push_str(&format!("\n• <b>{}</b> · {} ({:.0}%){mark}", html(&f.name), html(&m.left), m.pct));
        }
    }
    caption.push_str(&format!("\n\n{}", footer(&look)));
    for chat in chats(state) {
        document(&api, chat.id, pdf.to_vec(), &text::report_file(&look), caption.clone()).await?;
    }
    Ok(())
}

/// A captcha to type, sent to every linked chat; a reply answers it.
pub async fn captcha(state: &Arc<State>, id: &str, image: &[u8]) {
    let Some(api) = api(state) else { return };
    let lang = super::look(state, None).lang;
    let ask = words::pick(lang, "🔐 <b>A carrier asks for a captcha</b>\nReply to this message with the characters in the picture.", "🔐 <b>شركة الاتصالات تطلب رمز تحقق</b>\nرد على هذه الرسالة بالحروف التي في الصورة.");
    for chat in chats(state) {
        let form = Form::new()
            .text("chat_id", chat.id.to_string())
            .text("caption", ask)
            .text("parse_mode", "HTML")
            .text("reply_markup", json!({ "force_reply": true }).to_string())
            .part("photo", Part::bytes(image.to_vec()).file_name("captcha.png"));
        if let Ok(sent) = api.upload("sendPhoto", form).await
            && let Some(message) = sent["message_id"].as_i64()
            && let Ok(mut captchas) = state.telegram.captchas.lock()
        {
            captchas.insert((chat.id, message), id.to_string());
        }
    }
}

/// "Klue is linked" to every chat, for Settings → Telegram → Send a test.
pub async fn test(state: &Arc<State>) -> Result<(), String> {
    let api = api(state).ok_or("there's no bot token yet")?;
    let chats = chats(state);
    if chats.is_empty() {
        return Err("no chat is linked yet".into());
    }
    let look = super::look(state, None);
    let hello = if look.lang == Lang::Ar { format!("✅ <b>رسالة تجربة من Klue</b>\nالتنبيهات ستصل إلى هنا.\n\n{}", footer(&look)) } else { format!("✅ <b>A test from Klue</b>\nAlerts will arrive here.\n\n{}", footer(&look)) };
    for chat in chats {
        api.call("sendMessage", json!({ "chat_id": chat.id, "text": hello, "parse_mode": "HTML", "reply_markup": menu(state, look.lang), "disable_web_page_preview": true })).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use klue_core::store::Clock;

    use super::*;

    #[test]
    fn captions_carry_the_facts_and_fit_telegrams_limit() {
        let data = klue_engine::demo::data();
        for lang in [Lang::En, Lang::Ar] {
            let look = Look { lang, clock: Clock::H24, now: Utc::now(), server: Some("Home VPS".into()) };
            let caption = line_caption(&text::facts(&data.accounts[0], &look), &look);
            assert!(caption.contains("62.4") && caption.contains("klue.wkar.im") && caption.contains("Home VPS"), "{caption}");
            assert!(caption.chars().count() <= 1024);
        }
        let long = format!("{}\n<i>footer</i>", "line of text\n".repeat(200));
        let cut = truncate(long);
        assert!(cut.chars().count() <= 1024 && cut.ends_with("<i>footer</i>"));
    }
}
