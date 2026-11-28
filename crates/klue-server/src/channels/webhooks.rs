//! Webhooks: alerts and the daily report, in each service's own format. Discord gets
//! embeds posted as Klue with Klue's picture; the rest get their cards and blocks; any other
//! address gets Klue's JSON, signed if there's a signing secret.

use std::{sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use chrono::Utc;
use hmac::{Hmac, Mac};
use klue_core::{
    alerts::AlertEvent,
    store::{Account, Lang},
};
use klue_engine::remote::{Webhook, WebhookKind, secret};
use klue_report::{Look, SITE, words};
use serde_json::{Value, json};
use sha2::Sha256;
use wreq::multipart::{Form, Part};

use super::text::{self, Level};
use crate::state::State;

/// Klue's picture, for Discord's sender, Google Chat's header and ntfy's icon.
pub const LOGO: &str = "https://klue.wkar.im/assets/klue-256.png";
const YELLOW: u32 = 0xFFBE32;
const DANGER: u32 = 0xC8321E;

/// What a message says, before it's put in a service's format.
pub struct Content {
    /// "WE · Home internet", or the report's name.
    pub title: String,
    pub headline: String,
    pub lines: Vec<String>,
    /// Name and value pairs: a report's lines.
    pub fields: Vec<(String, String)>,
    pub colour: u32,
    pub warning: bool,
    /// What the JSON webhook says happened.
    pub event: &'static str,
    pub account: Option<Value>,
    pub lang: Lang,
    pub server: Option<String>,
}

impl Content {
    /// The title and headline on one line, without saying "Klue" twice.
    fn heading(&self) -> String {
        if self.title == "Klue" { self.headline.clone() } else { format!("{} · {}", self.title, self.headline) }
    }

    fn footer_plain(&self) -> String {
        let mut s = format!("{} Klue", text::sent_by(self.lang));
        if let Some(server) = &self.server {
            s.push_str(&format!(" · {server}"));
        }
        s
    }
}

fn colour_of(hex: &str) -> u32 {
    u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(YELLOW)
}

fn account_json(account: &Account, look: &Look) -> Value {
    let f = text::facts(account, look);
    json!({
        "id": account.id,
        "name": f.name,
        "carrier": account.carrier.id(),
        "line": format!("{:?}", account.line).to_lowercase(),
        "number": account.number,
        "package": f.package,
        "main": f.main.as_ref().map(|m| json!({ "name": m.name, "left": m.left, "total": m.total, "percent": (m.pct * 10.0).round() / 10.0, "low": m.low })),
        "renews_at": account.usage.as_ref().and_then(|u| u.renews_at),
        "balance_egp": account.usage.as_ref().and_then(|u| u.balance_egp),
        "checked_at": account.updated_at(),
    })
}

pub fn alert_content(account: &Account, event: &AlertEvent, look: &Look) -> Content {
    let a = text::alert(account, event, look);
    let mut lines = Vec::new();
    if let Some(pct) = a.pct {
        lines.push(format!("{} {:.0}%", text::blocks(pct), pct));
    }
    lines.extend(a.detail.clone());
    let warning = matches!(a.level, Level::Warning);
    Content {
        title: a.title,
        headline: a.headline,
        lines,
        fields: Vec::new(),
        colour: if warning { DANGER } else { colour_of(klue_report::carrier_colour(account.carrier)) },
        warning,
        event: match event {
            AlertEvent::QuotaLow { .. } => "quota_low",
            AlertEvent::RenewalSoon { .. } => "renewal_soon",
            AlertEvent::RenewalToday { .. } => "renewal_today",
        },
        account: Some(account_json(account, look)),
        lang: look.lang,
        server: look.server.clone(),
    }
}

pub fn report_content(lines: &[Account], look: &Look) -> Content {
    let lang = look.lang;
    let fields = lines
        .iter()
        .map(|a| {
            let f = text::facts(a, look);
            let name = format!("{} · {}", f.carrier_name, f.name);
            let mut value = match &f.main {
                Some(m) => format!("{} {} {} ({:.0}%){}", m.left, words::pick(lang, "of", "من"), m.total, m.pct, if m.low { " ⚠️" } else { "" }),
                None => "—".into(),
            };
            if let Some((_, when)) = &f.renews {
                value.push_str(&format!("\n{} {when}", words::pick(lang, "Renews", "يتجدد")));
            }
            (name, value)
        })
        .collect();
    Content {
        title: "Klue".into(),
        headline: text::report_title(look, true),
        lines: vec![words::pick(lang, "Every line, with its graphs, is in the PDF.", "كل الخطوط ورسومها في ملف PDF.").to_string()],
        fields,
        colour: YELLOW,
        warning: false,
        event: "daily_report",
        account: None,
        lang,
        server: look.server.clone(),
    }
}

pub fn test_content(look: &Look) -> Content {
    Content {
        title: "Klue".into(),
        headline: words::pick(look.lang, "✅ A test from Klue", "✅ رسالة تجربة من Klue").to_string(),
        lines: vec![words::pick(look.lang, "Alerts and reports will arrive here.", "ستصل التنبيهات والتقارير إلى هنا.").to_string()],
        fields: Vec::new(),
        colour: YELLOW,
        warning: false,
        event: "test",
        account: None,
        lang: look.lang,
        server: look.server.clone(),
    }
}

pub fn discord(c: &Content) -> Value {
    let mut description = c.lines.join("\n");
    description.push_str(&format!("\n\n{} [Klue]({SITE})", text::sent_by(c.lang)));
    let fields: Vec<Value> = c.fields.iter().take(25).map(|(name, value)| json!({ "name": name, "value": value, "inline": true })).collect();
    json!({
        "username": "Klue",
        "avatar_url": LOGO,
        "allowed_mentions": { "parse": [] },
        "embeds": [{
            "author": { "name": c.title, "url": SITE, "icon_url": LOGO },
            "title": c.headline,
            "url": SITE,
            "description": description.trim(),
            "color": c.colour,
            "fields": fields,
            "footer": { "text": c.footer_plain(), "icon_url": LOGO },
            "timestamp": Utc::now().to_rfc3339(),
        }],
    })
}

pub fn slack(c: &Content) -> Value {
    let mut blocks = vec![
        json!({ "type": "header", "text": { "type": "plain_text", "text": c.headline, "emoji": true } }),
        json!({ "type": "section", "text": { "type": "mrkdwn", "text": format!("*{}*\n{}", c.title, c.lines.join("\n")) } }),
    ];
    for chunk in c.fields.chunks(10) {
        let fields: Vec<Value> = chunk.iter().map(|(name, value)| json!({ "type": "mrkdwn", "text": format!("*{name}*\n{value}") })).collect();
        blocks.push(json!({ "type": "section", "fields": fields }));
    }
    let mut credit = format!("{} <{SITE}|Klue>", text::sent_by(c.lang));
    if let Some(server) = &c.server {
        credit.push_str(&format!(" · {server}"));
    }
    blocks.push(json!({ "type": "context", "elements": [{ "type": "image", "image_url": LOGO, "alt_text": "Klue" }, { "type": "mrkdwn", "text": credit }] }));
    json!({ "text": format!("{}: {}", c.title, c.headline), "blocks": blocks })
}

pub fn google_chat(c: &Content) -> Value {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut widgets = vec![json!({ "textParagraph": { "text": c.lines.iter().map(|l| esc(l)).collect::<Vec<_>>().join("<br>") } })];
    widgets.extend(c.fields.iter().map(|(name, value)| json!({ "decoratedText": { "topLabel": name, "text": esc(value).replace('\n', "<br>"), "wrapText": true } })));
    let mut credit = format!("{} <a href=\"{SITE}\">Klue</a>", text::sent_by(c.lang));
    if let Some(server) = &c.server {
        credit.push_str(&format!(" · {}", esc(server)));
    }
    json!({ "cardsV2": [{ "cardId": "klue", "card": {
        "header": { "title": c.headline, "subtitle": c.title, "imageUrl": LOGO, "imageType": "SQUARE" },
        "sections": [{ "widgets": widgets }, { "widgets": [{ "textParagraph": { "text": format!("<font color=\"#6B6968\">{credit}</font>") } }] }],
    }}]})
}

pub fn teams(c: &Content) -> Value {
    let mut body = vec![json!({ "type": "ColumnSet", "columns": [
        { "type": "Column", "width": "auto", "items": [{ "type": "Image", "url": LOGO, "size": "Small", "altText": "Klue" }] },
        { "type": "Column", "width": "stretch", "verticalContentAlignment": "Center", "items": [
            { "type": "TextBlock", "text": c.title, "weight": "Bolder", "wrap": true, "isSubtle": true },
            { "type": "TextBlock", "text": c.headline, "weight": "Bolder", "size": "Medium", "wrap": true, "spacing": "None", "color": if c.warning { "Attention" } else { "Default" } },
        ]},
    ]})];
    if !c.lines.is_empty() {
        body.push(json!({ "type": "TextBlock", "text": c.lines.join("\n\n"), "wrap": true }));
    }
    if !c.fields.is_empty() {
        body.push(json!({ "type": "FactSet", "facts": c.fields.iter().map(|(t, v)| json!({ "title": t, "value": v.replace('\n', " · ") })).collect::<Vec<_>>() }));
    }
    let mut credit = format!("{} [Klue]({SITE})", text::sent_by(c.lang));
    if let Some(server) = &c.server {
        credit.push_str(&format!(" · {server}"));
    }
    body.push(json!({ "type": "TextBlock", "text": credit, "size": "Small", "isSubtle": true, "wrap": true, "spacing": "Medium" }));
    json!({ "type": "message", "attachments": [{ "contentType": "application/vnd.microsoft.card.adaptive", "contentUrl": null, "content": {
        "$schema": "http://adaptivecards.io/schemas/adaptive-card.json", "type": "AdaptiveCard", "version": "1.4", "body": body,
    }}]})
}

/// ntfy's JSON form, which carries Arabic where headers can't: posted to the server's root
/// with the topic inside.
pub fn ntfy(c: &Content, topic: &str) -> Value {
    let mut message = c.lines.join("\n");
    for (name, value) in &c.fields {
        message.push_str(&format!("\n\n**{name}**\n{value}"));
    }
    message.push_str(&format!("\n\n{} [Klue]({SITE})", text::sent_by(c.lang)));
    json!({
        "topic": topic,
        "title": c.heading(),
        "message": message.trim(),
        "tags": [if c.warning { "warning" } else if c.event == "daily_report" { "bar_chart" } else { "calendar" }],
        "priority": if c.warning { 4 } else { 3 },
        "click": SITE,
        "icon": LOGO,
        "markdown": true,
    })
}

/// Klue's own JSON, for any address.
pub fn plain(c: &Content) -> Value {
    json!({
        "event": c.event,
        "sent_at": Utc::now(),
        "server": c.server,
        "title": c.title,
        "headline": c.headline,
        "details": c.lines,
        "line": c.account,
        "lines": c.fields.iter().map(|(name, value)| json!({ "name": name, "summary": value })).collect::<Vec<_>>(),
        "sent_by": { "name": "Klue", "url": SITE },
    })
}

/// `https://ntfy.sh/topic` as the server and the topic.
fn ntfy_parts(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches('/');
    let (base, topic) = url.rsplit_once('/')?;
    (base.contains("://") && !topic.is_empty()).then(|| (base.to_string(), topic.to_string()))
}

/// "sha256=…" over the body, for receivers to check it came from this server.
pub fn signature(secret: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes any key");
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    format!("sha256={}", digest.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// A header value as RFC 2047 when it isn't ASCII.
fn header_text(s: &str) -> String {
    if s.is_ascii() { s.to_string() } else { format!("=?UTF-8?B?{}?=", B64.encode(s)) }
}

/// Sends `content` to `hook`, with the PDF where the service takes files.
pub async fn send(state: &Arc<State>, hook: &Webhook, content: &Content, pdf: Option<(&str, &[u8])>) -> Result<(), String> {
    let url = state.vault.get(&secret::webhook_url(&hook.id)).ok_or("the webhook has no address")?;
    let client = klue_core::web_client(Duration::from_secs(60)).map_err(|e| e.to_string())?;
    let response = match hook.kind {
        WebhookKind::Discord => match pdf {
            Some((name, bytes)) => {
                let part = Part::bytes(bytes.to_vec()).file_name(name.to_string()).mime_str("application/pdf").map_err(|e| e.to_string())?;
                let form = Form::new().text("payload_json", discord(content).to_string()).part("files[0]", part);
                client.post(&url).multipart(form).send().await
            }
            None => client.post(&url).json(&discord(content)).send().await,
        },
        WebhookKind::Slack => client.post(&url).json(&slack(content)).send().await,
        WebhookKind::GoogleChat => client.post(&url).json(&google_chat(content)).send().await,
        WebhookKind::Teams => client.post(&url).json(&teams(content)).send().await,
        WebhookKind::Ntfy => {
            let (base, topic) = ntfy_parts(&url).ok_or("an ntfy address looks like https://ntfy.sh/your-topic")?;
            match pdf {
                // Files go up on their own, with the words in headers.
                Some((name, bytes)) => {
                    client
                        .put(&url)
                        .header("Filename", name)
                        .header("Title", header_text(&content.heading()))
                        .header("Message", header_text(&content.lines.join(" ")))
                        .header("Tags", "bar_chart")
                        .header("Click", SITE)
                        .header("Icon", LOGO)
                        .body(bytes.to_vec())
                        .send()
                        .await
                }
                None => client.post(&base).json(&ntfy(content, &topic)).send().await,
            }
        }
        WebhookKind::Json => {
            let mut body = plain(content);
            if let Some((name, bytes)) = pdf {
                body["report"] = json!({ "file_name": name, "pdf_base64": B64.encode(bytes) });
            }
            let bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
            let mut request = client.post(&url).header("Content-Type", "application/json").header("X-Klue-Event", content.event);
            if let Some(signing) = state.vault.get(&secret::webhook_signing(&hook.id)) {
                request = request.header("X-Klue-Signature", signature(&signing, &bytes));
            }
            request.body(bytes).send().await
        }
    }
    .map_err(|e| e.to_string())?;
    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        let detail = response.text().await.unwrap_or_default();
        Err(format!("{status}: {}", detail.chars().take(200).collect::<String>()))
    }
}

#[cfg(test)]
mod tests {
    use klue_core::store::Clock;

    use super::*;

    fn look(lang: Lang) -> Look {
        Look { lang, clock: Clock::H24, now: Utc::now(), server: Some("Home VPS".into()) }
    }

    #[test]
    fn each_service_gets_its_format_with_klues_name() {
        let data = klue_engine::demo::data();
        let account = &data.accounts[1];
        let event = AlertEvent::QuotaLow { account_id: account.id.clone(), bundle: account.main_bundle().unwrap().clone(), threshold: 5.0 };
        let c = alert_content(account, &event, &look(Lang::En));
        let d = discord(&c);
        assert_eq!(d["username"], "Klue");
        assert_eq!(d["avatar_url"], LOGO);
        assert_eq!(d["embeds"][0]["footer"]["text"], "Sent by Klue · Home VPS");
        assert_eq!(d["embeds"][0]["color"], DANGER);
        assert!(d["embeds"][0]["description"].as_str().unwrap().contains("[Klue](https://klue.wkar.im)"));
        assert!(slack(&c)["blocks"].to_string().contains("<https://klue.wkar.im|Klue>"));
        assert_eq!(google_chat(&c)["cardsV2"][0]["card"]["header"]["imageUrl"], LOGO);
        assert_eq!(teams(&c)["attachments"][0]["content"]["type"], "AdaptiveCard");
        assert_eq!(ntfy(&c, "lines")["topic"], "lines");
        assert_eq!(plain(&c)["event"], "quota_low");
        assert_eq!(plain(&c)["line"]["number"], account.number.as_str());

        let report = report_content(&data.accounts, &look(Lang::Ar));
        assert_eq!(discord(&report)["embeds"][0]["fields"].as_array().unwrap().len(), data.accounts.len());
        assert_eq!(ntfy_parts("https://ntfy.sh/my-lines/"), Some(("https://ntfy.sh".into(), "my-lines".into())));
        assert_eq!(ntfy_parts("my-lines"), None);
        // The signature receivers check: HMAC-SHA256 of the body.
        assert_eq!(signature("key", b"The quick brown fox jumps over the lazy dog"), "sha256=f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8");
        assert_eq!(header_text("plain"), "plain");
        assert!(header_text("تقرير").starts_with("=?UTF-8?B?"));
    }
}
