//! Email: alerts, and the daily report with its table of lines and the PDF attached. HTML
//! made of tables and inline styles, as mail apps need, with plain text alongside.

use std::{sync::Arc, time::Duration};

use klue_core::{
    alerts::AlertEvent,
    store::{Account, Lang},
};
use klue_engine::remote::{EmailSettings, SmtpSecurity, secret};
use klue_report::{Look, SITE, words};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Attachment, Mailbox, MultiPart, SinglePart, header::ContentType},
    transport::smtp::authentication::Credentials,
};

use super::text::{self, Level};
use crate::state::State;

const PAPER: &str = "#F5F4F1";
const INK: &str = "#1D1B1B";
const MUTED: &str = "#6B6968";
const HAIRLINE: &str = "#E2DFDA";
const TRACK: &str = "#ECEAE6";
const YELLOW: &str = "#FFBE32";
const YELLOW_INK: &str = "#8A5B00";
const DANGER: &str = "#C8321E";

pub struct Mail {
    pub subject: String,
    pub html: String,
    pub text: String,
    pub attachment: Option<(String, Vec<u8>)>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn font(lang: Lang) -> &'static str {
    if lang == Lang::Ar { "'Google Sans',Tahoma,Arial,sans-serif" } else { "'DM Sans',Helvetica,Arial,sans-serif" }
}

/// A bar `pct` full, as a table so every mail app draws it.
fn bar(pct: f64, colour: &str) -> String {
    let filled = pct.clamp(0.0, 100.0).round() as u32;
    let rest = 100 - filled;
    let mut cells = String::new();
    if filled > 0 {
        cells.push_str(&format!(r#"<td width="{filled}%" style="background:{colour};height:8px;font-size:0;line-height:0;border-radius:4px">&nbsp;</td>"#));
    }
    if rest > 0 {
        cells.push_str(&format!(r#"<td width="{rest}%" style="background:{TRACK};height:8px;font-size:0;line-height:0;border-radius:4px">&nbsp;</td>"#));
    }
    format!(r#"<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="border-collapse:separate"><tr>{cells}</tr></table>"#)
}

/// Klue's page around `content`: the mark, a white card, and "Sent by Klue" under it.
fn page(look: &Look, subject: &str, content: &str) -> String {
    let lang = look.lang;
    let (dir, align) = if lang == Lang::Ar { ("rtl", "right") } else { ("ltr", "left") };
    let f = font(lang);
    // Each piece keeps its own direction, so Arabic dates and Latin names don't run together.
    let server = look.server.as_deref().map(|s| format!(r#" · <span dir="auto">{}</span>"#, esc(s))).unwrap_or_default();
    let stamp = format!(r#"<span dir="{dir}">{}</span>"#, esc(&words::stamp(look.now, look.clock, lang)));
    format!(
        r#"<!doctype html>
<html lang="{lang_code}" dir="{dir}">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{subject}</title></head>
<body style="margin:0;padding:0;background:{PAPER}">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background:{PAPER}" dir="{dir}">
<tr><td align="center" style="padding:28px 12px">
<table role="presentation" width="600" cellpadding="0" cellspacing="0" style="width:100%;max-width:600px">
<tr><td style="padding:0 4px 18px;text-align:{align}">
<table role="presentation" cellpadding="0" cellspacing="0" dir="{dir}"><tr>
<td style="padding-{gap}:10px"><img src="cid:klue-logo" width="28" height="28" alt="" style="display:block;border:0;border-radius:6px"></td>
<td style="font:800 18px/1 {f};color:{INK}">Klue</td>
</tr></table>
</td></tr>
<tr><td style="background:#FFFFFF;border:1px solid {HAIRLINE};border-radius:16px;padding:28px;text-align:{align};font:400 14px/1.5 {f};color:{INK}">
{content}
</td></tr>
<tr><td style="padding:16px 4px;text-align:{align};font:400 12px/1.6 {f};color:{MUTED}">{sent_by} <a href="{SITE}" dir="ltr" style="color:{INK};font-weight:700;text-decoration:none">Klue</a>{server} · {stamp}</td></tr>
</table>
</td></tr>
</table>
</body>
</html>"#,
        lang_code = if lang == Lang::Ar { "ar" } else { "en" },
        subject = esc(subject),
        gap = if lang == Lang::Ar { "left" } else { "right" },
        sent_by = text::sent_by(lang),
    )
}

pub fn alert_mail(account: &Account, event: &AlertEvent, look: &Look) -> Mail {
    let lang = look.lang;
    let f = font(lang);
    let a = text::alert(account, event, look);
    let facts = text::facts(account, look);
    let (kicker, colour) = match a.level {
        Level::Warning => (words::pick(lang, "RUNNING LOW", "الرصيد ينفد"), DANGER),
        Level::Reminder => (words::pick(lang, "RENEWAL", "التجديد"), YELLOW_INK),
    };
    let mut content = format!(
        r#"<div style="font:700 11px/1 {f};letter-spacing:.06em;color:{colour}">{kicker}</div>
<h1 style="margin:8px 0 6px;font:800 22px/1.3 {f};color:{INK}">{headline}</h1>
<p style="margin:0 0 18px;font:400 14px/1.5 {f};color:{MUTED}">{title} · <span dir="ltr">{number}</span></p>"#,
        headline = esc(&a.headline),
        title = esc(&a.title),
        number = esc(&facts.number),
    );
    if let Some(pct) = a.pct {
        content.push_str(&bar(pct, if matches!(a.level, Level::Warning) { DANGER } else { YELLOW }));
        content.push_str(&format!(r#"<p style="margin:8px 0 0;font:400 13px/1.5 {f};color:{MUTED}">{:.0}% {}</p>"#, pct, words::pick(lang, "left", "متبقية")));
    }
    if let Some(detail) = &a.detail {
        content.push_str(&format!(r#"<p style="margin:16px 0 0;font:600 14px/1.5 {f};color:{INK}">{}</p>"#, esc(detail)));
    }
    let subject = format!("{}: {}", account.display_name(), a.headline);
    let mut text = format!("{}\n{}\n", a.title, a.headline);
    if let Some(detail) = &a.detail {
        text.push_str(detail);
        text.push('\n');
    }
    text.push_str(&format!("\n{} Klue ({SITE})\n", text::sent_by(lang)));
    Mail { html: page(look, &subject, &content), subject, text, attachment: None }
}

/// The daily report: a table of every line, and the PDF.
pub fn report_mail(lines: &[Account], look: &Look, pdf: Vec<u8>) -> Mail {
    let lang = look.lang;
    let f = font(lang);
    let align = if lang == Lang::Ar { "right" } else { "left" };
    let end = if lang == Lang::Ar { "left" } else { "right" };
    let subject = text::report_title(look, true);
    let head = |s: &str, a: &str| format!(r#"<td style="padding:0 0 8px;font:700 10px/1 {f};letter-spacing:.06em;color:{MUTED};text-align:{a};border-bottom:2px solid {INK}">{s}</td>"#);
    let mut rows = String::new();
    let mut text = format!("{subject}\n\n");
    for account in lines {
        let x = text::facts(account, look);
        let (left, of, pct, low) = match &x.main {
            Some(m) => (m.left.clone(), m.total.clone(), m.pct, m.low),
            None => ("—".into(), String::new(), 0.0, false),
        };
        let renews = x.renews.as_ref().map(|(_, when)| when.clone()).unwrap_or_else(|| "—".into());
        let balance = x.balance.clone().unwrap_or_else(|| "—".into());
        let cell = |s: String, a: &str| format!(r#"<td style="padding:12px 0;border-bottom:1px solid {HAIRLINE};vertical-align:top;text-align:{a}">{s}</td>"#);
        rows.push_str("<tr>");
        rows.push_str(&cell(
            format!(r#"<div style="font:700 14px/1.3 {f};color:{INK}">{}</div><div style="font:400 12px/1.4 {f};color:{MUTED}">{} · <span dir="ltr">{}</span></div>"#, esc(&x.name), x.carrier_name, esc(&x.number)),
            align,
        ));
        rows.push_str(&cell(
            format!(r#"<div style="font:800 14px/1.3 {f};color:{}">{}</div><div style="font:400 12px/1.4 {f};color:{MUTED}">{} {}</div>"#, if low { DANGER } else { INK }, esc(&left), words::pick(lang, "of", "من"), esc(&of)),
            align,
        ));
        rows.push_str(&cell(format!(r#"<div style="width:84px">{}</div><div style="font:600 12px/1.6 {f};color:{MUTED}">{:.0}%</div>"#, bar(pct, if low { DANGER } else { YELLOW }), pct), align));
        rows.push_str(&cell(format!(r#"<div style="font:600 13px/1.3 {f};color:{YELLOW_INK}">{}</div>"#, esc(&renews)), align));
        rows.push_str(&cell(format!(r#"<div style="font:600 13px/1.3 {f};color:{INK}">{}</div>"#, esc(&balance)), end));
        rows.push_str("</tr>");
        text.push_str(&format!("{} · {}: {left} {} ({pct:.0}%), {renews}, {balance}\n", x.carrier_name, x.name, if of.is_empty() { String::new() } else { format!("{} {of}", words::pick(lang, "of", "من")) }));
    }
    let count = if lang == Lang::Ar { format!("{} خط", lines.len()) } else { format!("{} lines", lines.len()) };
    let content = format!(
        r#"<div style="font:700 11px/1 {f};letter-spacing:.06em;color:{YELLOW_INK}">{kicker}</div>
<h1 style="margin:8px 0 4px;font:800 24px/1.25 {f};color:{INK}">{title}</h1>
<p style="margin:0 0 22px;font:400 14px/1.5 {f};color:{MUTED}">{date} · {count}</p>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;font:400 13px/1.4 {f}">
<tr>{h1}{h2}{h3}{h4}{h5}</tr>
{rows}
</table>
<p style="margin:22px 0 0;font:400 13px/1.5 {f};color:{MUTED}">{attached}</p>"#,
        kicker = words::pick(lang, "DAILY REPORT", "التقرير اليومي"),
        title = words::pick(lang, "Your lines today", "خطوطك اليوم"),
        date = esc(&words::date_long(look.now, lang)),
        h1 = head(words::pick(lang, "LINE", "الخط"), align),
        h2 = head(words::pick(lang, "LEFT", "المتبقي"), align),
        h3 = head(words::pick(lang, "SHARE", "النسبة"), align),
        h4 = head(words::pick(lang, "RENEWS", "التجديد"), align),
        h5 = head(words::pick(lang, "BALANCE", "الرصيد"), end),
        attached = words::pick(lang, "Every line, with its graphs, is in the PDF attached.", "كل الخطوط ورسومها في ملف PDF المرفق."),
    );
    text.push_str(&format!("\n{}\n{} Klue ({SITE})\n", words::pick(lang, "Every line, with its graphs, is in the PDF attached.", "كل الخطوط ورسومها في ملف PDF المرفق."), text::sent_by(lang)));
    Mail { html: page(look, &subject, &content), subject, text, attachment: Some((text::report_file(look), pdf)) }
}

pub fn test_mail(look: &Look) -> Mail {
    let lang = look.lang;
    let f = font(lang);
    let subject = words::pick(lang, "A test from Klue", "رسالة تجربة من Klue").to_string();
    let content = format!(
        r#"<h1 style="margin:0 0 6px;font:800 22px/1.3 {f};color:{INK}">✅ {subject}</h1><p style="margin:0;font:400 14px/1.5 {f};color:{MUTED}">{body}</p>"#,
        body = words::pick(lang, "Email from Klue Server works. Alerts and the daily report will come to this address.", "البريد من Klue Server يعمل. ستصل التنبيهات والتقرير اليومي إلى هذا العنوان.")
    );
    Mail { html: page(look, &subject, &content), text: format!("{subject}\n"), subject, attachment: None }
}

/// The message as sent: plain text and HTML, the logo inline, and the attachment.
pub fn message(settings: &EmailSettings, mail: Mail) -> Result<Message, String> {
    let from_text = if settings.from.trim().is_empty() { settings.username.clone() } else { settings.from.clone() };
    let from: Mailbox = from_text.parse().map_err(|e| format!("the sender address: {e}"))?;
    if settings.to.is_empty() {
        return Err("there's no recipient".into());
    }
    let mut builder = Message::builder().from(from).subject(mail.subject);
    for to in &settings.to {
        builder = builder.to(to.parse::<Mailbox>().map_err(|e| format!("{to}: {e}"))?);
    }
    let logo = klue_report::logo_png(56).unwrap_or_default();
    let body = MultiPart::alternative().singlepart(SinglePart::plain(mail.text)).multipart(
        MultiPart::related()
            .singlepart(SinglePart::html(mail.html))
            .singlepart(Attachment::new_inline("klue-logo".to_string()).body(logo, ContentType::parse("image/png").map_err(|e| e.to_string())?)),
    );
    let body = match mail.attachment {
        Some((name, bytes)) => MultiPart::mixed().multipart(body).singlepart(Attachment::new(name).body(bytes, ContentType::parse("application/pdf").map_err(|e| e.to_string())?)),
        None => body,
    };
    builder.multipart(body).map_err(|e| e.to_string())
}

pub async fn send(state: &Arc<State>, mail: Mail) -> Result<(), String> {
    let settings = state.settings().email;
    if settings.host.trim().is_empty() {
        return Err("there's no mail server yet".into());
    }
    let message = message(&settings, mail)?;
    let builder = match settings.security {
        SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.host).map_err(|e| e.to_string())?,
        SmtpSecurity::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host).map_err(|e| e.to_string())?,
        SmtpSecurity::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.host),
    };
    let mut builder = builder.port(settings.port).timeout(Some(Duration::from_secs(30)));
    if !settings.username.is_empty() {
        builder = builder.credentials(Credentials::new(settings.username.clone(), state.vault.get(secret::SMTP_PASSWORD).unwrap_or_default()));
    }
    builder.build().send(message).await.map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use klue_core::{alerts::AlertEvent, store::Clock};

    use super::*;

    #[test]
    fn mails_have_text_html_the_logo_and_the_report() {
        let data = klue_engine::demo::data();
        let settings = EmailSettings { host: "smtp.example.com".into(), username: "klue@example.com".into(), to: vec!["me@example.com".into()], ..Default::default() };
        for lang in [Lang::En, Lang::Ar] {
            let look = Look { lang, clock: Clock::H24, now: Utc::now(), server: Some("Home VPS".into()) };
            let account = &data.accounts[1];
            let bundle = account.main_bundle().unwrap().clone();
            let event = AlertEvent::QuotaLow { account_id: account.id.clone(), bundle, threshold: 5.0 };
            let alert = alert_mail(account, &event, &look);
            assert!(alert.html.contains("cid:klue-logo") && alert.html.contains(SITE) && alert.html.contains("Home VPS"));
            let report = report_mail(&data.accounts, &look, b"%PDF-1.7 test".to_vec());
            assert!(report.html.contains("02 2358 4417") && report.text.contains("62.4"));
            let sent = String::from_utf8(message(&settings, report).unwrap().formatted()).unwrap();
            for part in ["multipart/mixed", "multipart/alternative", "multipart/related", "text/plain", "text/html", "image/png", "application/pdf", "klue-report-"] {
                assert!(sent.contains(part), "{part} missing");
            }
            if std::env::var("KLUE_MAIL_OUT").is_ok() {
                let dir = std::env::var("KLUE_MAIL_OUT").unwrap();
                let tag = if lang == Lang::Ar { "ar" } else { "en" };
                std::fs::write(format!("{dir}/alert-{tag}.html"), alert_mail(account, &event, &look).html.replace("cid:klue-logo", "logo.png")).unwrap();
                std::fs::write(format!("{dir}/report-{tag}.html"), report_mail(&data.accounts, &look, Vec::new()).html.replace("cid:klue-logo", "logo.png")).unwrap();
            }
        }
    }
}
