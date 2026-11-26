//! Export and import: numbers to a file and back, to move them to another computer.

use std::path::PathBuf;

use iced::{
    Alignment, Length, Task,
    font::Weight,
    widget::{button, column, container, text_input},
};
use klue_core::{
    store::{Account, AppData},
    transfer::{self, Export, TransferError},
};

use super::{
    Ctx, El, gap, hline, tile,
    modals::{self, Modal, dialog, label},
};
use crate::{
    app::{App, Message},
    assets::{self, Icon},
    i18n, theme,
};

/// Focused as either dialog opens.
pub const PASSPHRASE: &str = "transfer-passphrase";

pub struct ExportForm {
    ids: Vec<String>,
    /// Which of them go in the file; all to begin with.
    picks: Vec<bool>,
    with_passwords: bool,
    passphrase: String,
    show: bool,
    phase: Phase,
}

pub struct ImportForm {
    path: PathBuf,
    file: Result<Export, TransferError>,
    /// Which of the file's lines to take, by index.
    picks: Vec<bool>,
    passphrase: String,
    show: bool,
    phase: Phase,
}

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    Ready,
    Busy,
    /// Exported to this file.
    Saved(PathBuf),
    Failed(String),
}

/// What happens to a line of the file here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fit {
    New,
    /// The same number is here: it gets the file's history.
    Here,
    /// New, but the file has no password for it.
    NoPassword,
    /// A kind of line this version can't read.
    Unsupported,
}

#[derive(Debug, Clone)]
pub enum Msg {
    WithPasswords,
    Passphrase(String),
    ShowPassphrase,
    /// Export: pick where, then write the file.
    Save,
    Saved(Result<Option<PathBuf>, String>),
    /// Export several: put all of them in the file, or none.
    All(bool),
    /// Import: pick a file.
    Pick,
    Picked(Option<(PathBuf, Box<Result<Export, TransferError>>)>),
    Toggle(usize),
    Import,
    /// The passwords, opened with the passphrase.
    Opened(Result<Vec<Option<String>>, TransferError>),
}

fn wrap(msg: Msg) -> Message {
    Message::Modal(modals::Msg::Transfer(msg))
}

impl ExportForm {
    pub fn new(ids: Vec<String>) -> Self {
        let picks = vec![true; ids.len()];
        Self { ids, picks, with_passwords: true, passphrase: String::new(), show: false, phase: Phase::Ready }
    }

    fn picked(&self) -> Vec<String> {
        self.ids.iter().zip(&self.picks).filter(|(_, picked)| **picked).map(|(id, _)| id.clone()).collect()
    }

    fn ready(&self) -> bool {
        self.picks.iter().any(|p| *p) && (!self.with_passwords || self.passphrase.chars().count() >= transfer::MIN_PASSPHRASE)
    }
}

impl ImportForm {
    fn new(path: PathBuf, file: Result<Export, TransferError>, data: &AppData) -> Self {
        let picks = match &file {
            Ok(export) => export.lines.iter().map(|l| matches!(fit(l, data), Fit::New | Fit::Here)).collect(),
            Err(_) => Vec::new(),
        };
        Self { path, file, picks, passphrase: String::new(), show: false, phase: Phase::Ready }
    }

    /// The picked lines that are new here; their passwords need the passphrase.
    fn needs_passphrase(&self, data: &AppData) -> bool {
        let Ok(export) = &self.file else { return false };
        export.lines.iter().zip(&self.picks).any(|(l, picked)| *picked && fit(l, data) == Fit::New)
    }
}

fn fit(line: &transfer::Line, data: &AppData) -> Fit {
    if !line.carrier.supports_line(line.line) {
        Fit::Unsupported
    } else if data.accounts.iter().any(|a| line.is(a)) {
        Fit::Here
    } else if line.has_password() {
        Fit::New
    } else {
        Fit::NoPassword
    }
}

/// "klue-orange-01200000000.klue", or "klue-numbers-2026-11-26.klue" for several.
fn file_name(accounts: &[&Account]) -> String {
    match accounts {
        [one] => format!("klue-{}-{}.{}", format!("{:?}", one.carrier).to_lowercase(), one.number, transfer::EXTENSION),
        _ => format!("klue-numbers-{}.{}", chrono::Local::now().format("%Y-%m-%d"), transfer::EXTENSION),
    }
}

/// The native Open dialog, then the file read.
pub fn pick() -> Task<Message> {
    Task::perform(
        async {
            let handle = rfd::AsyncFileDialog::new().add_filter("Klue", &[transfer::EXTENSION]).pick_file().await?;
            let path = handle.path().to_path_buf();
            let file = std::fs::read(&path).map_err(|e| {
                tracing::warn!("couldn't read {}: {e}", path.display());
                TransferError::NotAnExport
            });
            Some((path, Box::new(file.and_then(|bytes| Export::parse(&bytes)))))
        },
        |picked| wrap(Msg::Picked(picked)),
    )
}

pub fn update(app: &mut App, msg: Msg) -> Task<Message> {
    let shared = app.shared.clone();
    match (&mut app.modal, msg) {
        (Modal::Export(f), Msg::WithPasswords) => f.with_passwords = !f.with_passwords,
        (Modal::Export(f), Msg::Passphrase(p)) => f.passphrase = p,
        (Modal::Export(f), Msg::ShowPassphrase) => f.show = !f.show,
        (Modal::Export(f), Msg::Toggle(i)) if f.phase != Phase::Busy => {
            if let Some(pick) = f.picks.get_mut(i) {
                *pick = !*pick;
            }
        }
        (Modal::Export(f), Msg::All(on)) if f.phase != Phase::Busy => f.picks.iter_mut().for_each(|p| *p = on),
        (Modal::Export(f), Msg::Save) if f.ready() && f.phase != Phase::Busy => {
            let ids = f.picked();
            let accounts: Vec<&Account> = ids.iter().filter_map(|id| app.snap.data.account(id)).collect();
            let name = file_name(&accounts);
            let passphrase = f.with_passwords.then(|| f.passphrase.clone());
            f.phase = Phase::Busy;
            return Task::perform(
                async move {
                    let Some(handle) = rfd::AsyncFileDialog::new().set_file_name(&name).add_filter("Klue", &[transfer::EXTENSION]).save_file().await else {
                        return Ok(None);
                    };
                    let path = handle.path().to_path_buf();
                    let bytes = tokio::task::spawn_blocking(move || shared.export(&ids, passphrase.as_deref())).await.map_err(|e| e.to_string())?;
                    let bytes = bytes.map_err(|e| e.to_string())?;
                    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                    Ok(Some(path))
                },
                |r| wrap(Msg::Saved(r)),
            );
        }
        (Modal::Export(f), Msg::Saved(result)) => {
            f.phase = match result {
                Ok(Some(path)) => Phase::Saved(path),
                Ok(None) => Phase::Ready,
                Err(e) => {
                    tracing::warn!("export failed: {e}");
                    Phase::Failed(e)
                }
            };
        }

        (_, Msg::Pick) => return pick(),
        (_, Msg::Picked(Some((path, file)))) => {
            let form = ImportForm::new(path, *file, &app.snap.data);
            let focus = form.needs_passphrase(&app.snap.data);
            app.modal = Modal::Import(form);
            let focus = if focus { iced::widget::operation::focus(PASSPHRASE) } else { Task::none() };
            return Task::batch([modals::top(), focus]);
        }
        (Modal::Import(f), Msg::Toggle(i)) => {
            if let Some(pick) = f.picks.get_mut(i) {
                *pick = !*pick;
            }
        }
        (Modal::Import(f), Msg::Passphrase(p)) => {
            f.passphrase = p;
            f.phase = Phase::Ready;
        }
        (Modal::Import(f), Msg::ShowPassphrase) => f.show = !f.show,
        (Modal::Import(f), Msg::Import) if f.phase != Phase::Busy => {
            let Ok(export) = &f.file else { return Task::none() };
            if !f.needs_passphrase(&app.snap.data) {
                let none = vec![None; export.lines.len()];
                return update(app, Msg::Opened(Ok(none)));
            }
            f.phase = Phase::Busy;
            let (export, passphrase) = (export.clone(), f.passphrase.clone());
            return Task::perform(
                async move { tokio::task::spawn_blocking(move || export.passwords(&passphrase)).await.unwrap_or(Err(TransferError::NotAnExport)) },
                |r| wrap(Msg::Opened(r)),
            );
        }
        (Modal::Import(f), Msg::Opened(result)) => {
            let Ok(export) = &f.file else { return Task::none() };
            match result {
                Ok(passwords) => {
                    let lines: Vec<(transfer::Line, Option<String>)> = export
                        .lines
                        .iter()
                        .zip(passwords)
                        .zip(&f.picks)
                        .filter(|((line, _), picked)| **picked && matches!(fit(line, &app.snap.data), Fit::New | Fit::Here))
                        .map(|((line, password), _)| (line.clone(), password))
                        .collect();
                    let ids = shared.import(lines);
                    app.modal = Modal::None;
                    app.snap = shared.snapshot();
                    if let Some(id) = ids.into_iter().find(|id| app.snap.data.account(id).is_some_and(|a| !a.archived)) {
                        return Task::done(Message::SelectAccount(id));
                    }
                }
                Err(e) => {
                    f.phase = Phase::Failed(i18n::transfer_error(&e, app.snap.data.settings.lang).to_string());
                    if e == TransferError::WrongPassphrase {
                        return iced::widget::operation::focus(PASSPHRASE);
                    }
                }
            }
        }
        _ => {}
    }
    Task::none()
}

// ---------------------------------------------------------------------------------------
// Views

/// A number as a row: its tile, its name and its number.
fn line_row<'a>(ctx: &Ctx<'a>, carrier: klue_core::Carrier, name: String, number: &str, note: Option<(String, bool)>, end: Option<El<'a>>) -> El<'a> {
    let p = ctx.p;
    let t = assets::tile(carrier);
    let mut text = column![ctx.semi(name, 14.0), ctx.num(klue_core::format::pretty_number(number), 12.0, Weight::Normal, p.mu)].spacing(1).align_x(ctx.start());
    if let Some((note, good)) = note {
        text = text.push(ctx.text(note, 11.5, Weight::Semibold, if good { p.ac_tx } else { p.mu }));
    }
    let mut items = vec![tile(carrier, 36.0, t.small * 1.2, 10.0, false), container(text).width(Length::Fill).align_x(ctx.start()).into()];
    items.extend(end);
    column![container(ctx.row(items).spacing(12)).padding([10, 0]), hline(p.dvs, 1.0)].into()
}

/// Why the same account on several computers can get it blocked.
fn devices<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let text = column![ctx.bold(s.devices_title, 14.0), ctx.body(s.devices_body, 12.5).line_height(1.45)].spacing(3).align_x(ctx.start()).width(Length::Fill);
    container(ctx.row(vec![ctx.icon(Icon::Info, 18.0, p.ac_tx), text.into()]).spacing(12).align_y(Alignment::Start))
        .padding([12, 14])
        .style(move |_| theme::fill(theme::fade(p.ac, 0.14), 10.0))
        .into()
}

/// The passphrase field with its eye, for both dialogs.
fn passphrase<'a>(ctx: &Ctx<'a>, value: &'a str, show: bool, error: bool, hint: String) -> El<'a> {
    let p = ctx.p;
    let field = text_input("", value)
        .id(PASSPHRASE)
        .secure(!show)
        .on_input(|t| wrap(Msg::Passphrase(t)))
        .padding([9, 10])
        .size(14)
        .style(theme::field(p, error));
    let eye = button(container(ctx.icon(if show { Icon::EyeOff } else { Icon::Eye }, 16.0, p.mu)).center(Length::Fill))
        .width(38)
        .height(38)
        .padding(0)
        .style(theme::ghost(p.mu, p.sf, 9.0))
        .on_press(wrap(Msg::ShowPassphrase));
    column![
        label(ctx, ctx.s.passphrase),
        gap(6.0),
        ctx.row(vec![field.into(), eye.into()]).spacing(6),
        gap(6.0),
        ctx.text(hint, 12.0, Weight::Normal, if error { p.danger } else { p.mu })
    ]
    .align_x(ctx.start())
    .into()
}

pub fn export_view<'a>(ctx: &Ctx<'a>, f: &'a ExportForm) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let accounts: Vec<&Account> = f.ids.iter().filter_map(|id| ctx.app.snap.data.account(id)).collect();
    let many = accounts.len() > 1;
    let busy = f.phase == Phase::Busy;
    // Several numbers each get a switch; archived ones say so.
    let rows: Vec<El<'a>> = accounts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let note = a.archived.then(|| (s.archived_rail.to_string(), false));
            let end = many.then(|| ctx.toggle(f.picks[i], wrap(Msg::Toggle(i))));
            line_row(ctx, a.carrier, a.display_name(), &a.number, note, end)
        })
        .collect();
    let list: El<'a> = if many {
        let picked = f.picks.iter().filter(|p| **p).count();
        let all = picked == f.picks.len();
        let head = ctx.row(vec![
            container(ctx.muted(i18n::picked_of(picked, f.picks.len(), ctx.lang), 12.5)).width(Length::Fill).align_x(ctx.start()).into(),
            ctx.link(if all { s.select_none } else { s.select_all }, wrap(Msg::All(!all))).into(),
        ]);
        column![head, hline(p.dvs, 1.0), column(rows)].spacing(4).into()
    } else {
        column![hline(p.dvs, 1.0), column(rows)].into()
    };
    let toggle = ctx.row(vec![
        ctx.toggle(f.with_passwords, wrap(Msg::WithPasswords)),
        column![ctx.semi(s.with_passwords, 13.5), ctx.muted(s.with_passwords_hint, 12.0)].spacing(2).width(Length::Fill).align_x(ctx.start()).into(),
    ])
    .spacing(12)
    .align_y(Alignment::Start);
    let mut body = column![list, ctx.muted(s.export_contents, 13.0), toggle].spacing(14).align_x(ctx.start());
    if f.with_passwords {
        body = body.push(passphrase(ctx, &f.passphrase, f.show, false, s.passphrase_hint.to_string()));
    }
    body = body.push(devices(ctx));
    match &f.phase {
        Phase::Saved(path) => {
            let done = ctx.row(vec![ctx.icon(Icon::Check, 16.0, p.ac_tx), ctx.semi(i18n::saved_to(&path.display().to_string(), ctx.lang), 13.0).into()]).spacing(10);
            body = body.push(container(done).padding([10, 14]).width(Length::Fill).style(theme::bg_line(p.sf, p.ac, 10.0)));
        }
        Phase::Failed(e) => body = body.push(ctx.text(e.clone(), 12.5, Weight::Semibold, p.danger)),
        _ => {}
    }

    // Once it's saved there's nothing to cancel.
    let foot = match &f.phase {
        Phase::Saved(_) => ctx.row(vec![super::fill_x(), ctx.primary(s.done, Some(Message::CloseModal)).into()]),
        _ => ctx.row(vec![
            super::fill_x(),
            ctx.outline(s.cancel, Some(Message::CloseModal)).into(),
            ctx.primary(if busy { s.saving } else { s.save_file }, (f.ready() && !busy).then(|| wrap(Msg::Save))).into(),
        ]),
    }
    .spacing(8);
    let (kicker, title) = match accounts.as_slice() {
        [one] => (format!("{} · {}", i18n::carrier_name(one.carrier, ctx.lang), one.pretty_number()), s.export_number),
        _ => (s.your_numbers.to_string(), s.export_numbers),
    };
    dialog(ctx, 540.0, kicker, title.to_string(), Some(Message::CloseModal), body.into(), foot.into())
}

pub fn import_view<'a>(ctx: &Ctx<'a>, f: &'a ImportForm) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let data = &ctx.app.snap.data;
    let name = f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let busy = f.phase == Phase::Busy;

    let (body, primary): (El<'a>, El<'a>) = match &f.file {
        Err(e) => {
            let what = column![ctx.bold(name, 14.0), ctx.body(i18n::transfer_error(e, ctx.lang), 13.0)].spacing(2).align_x(ctx.start()).width(Length::Fill);
            let body = container(ctx.row(vec![ctx.icon(Icon::Warning, 18.0, p.danger), what.into()]).spacing(12).align_y(Alignment::Start))
                .padding([12, 14])
                .style(move |_| theme::fill(theme::fade(p.danger, 0.12), 10.0));
            (body.into(), ctx.primary(s.choose_file, Some(wrap(Msg::Pick))).into())
        }
        Ok(export) => {
            let head = ctx.row(vec![
                container(ctx.icon(Icon::Download, 18.0, p.ac_tx)).center(36).style(move |_| theme::fill(theme::fade(p.ac, 0.2), 10.0)).into(),
                column![ctx.semi(name, 14.0), ctx.muted(i18n::exported_on(export.exported_at, &export.app_version, ctx.lang), 12.0)].spacing(1).align_x(ctx.start()).width(Length::Fill).into(),
            ])
            .spacing(12);
            let rows: Vec<El<'a>> = export
                .lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let fit = fit(line, data);
                    let note = match fit {
                        Fit::New => (s.line_new, true),
                        Fit::Here => (s.line_here, true),
                        Fit::NoPassword => (s.line_no_password, false),
                        Fit::Unsupported => (s.line_unsupported, false),
                    };
                    let end = matches!(fit, Fit::New | Fit::Here).then(|| ctx.toggle(f.picks[i], wrap(Msg::Toggle(i))));
                    line_row(ctx, line.carrier, line.display_name(), &line.number, Some((note.0.to_string(), note.1)), end)
                })
                .collect();
            let mut body = column![head, column![hline(p.dvs, 1.0), column(rows)]].spacing(14).align_x(ctx.start());
            let needs = f.needs_passphrase(data);
            if needs {
                let (error, hint) = match &f.phase {
                    Phase::Failed(e) => (true, e.clone()),
                    _ => (false, s.import_passphrase_hint.to_string()),
                };
                body = body.push(passphrase(ctx, &f.passphrase, f.show, error, hint));
            }
            body = body.push(devices(ctx));
            let picked = f.picks.iter().any(|p| *p);
            let ready = picked && !busy && (!needs || !f.passphrase.is_empty());
            (body.into(), ctx.primary(if busy { s.importing } else { s.import }, ready.then(|| wrap(Msg::Import))).into())
        }
    };
    let foot = ctx.row(vec![super::fill_x(), ctx.outline(s.cancel, Some(Message::CloseModal)).into(), primary]).spacing(8);
    dialog(ctx, 540.0, s.your_numbers.to_string(), s.import_numbers.to_string(), Some(Message::CloseModal), body, foot.into())
}

/// Settings: export every number, or import a file.
pub fn settings_section<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let data = &ctx.app.snap.data;
    let ids: Vec<String> = data.carriers().into_iter().flat_map(|c| data.accounts.iter().filter(move |a| a.carrier == c)).map(|a| a.id.clone()).collect();
    let row = |title: &'a str, hint: &'a str, action: El<'a>| -> El<'a> {
        container(
            ctx.row(vec![column![ctx.semi(title, 13.5), ctx.muted(hint, 12.0)].spacing(2).width(Length::Fill).align_x(ctx.start()).into(), action])
                .spacing(12),
        )
        .padding([10, 0])
        .into()
    };
    let export = ctx.outline(s.export_button, (!ids.is_empty()).then(|| Message::OpenModal(crate::app::ModalKind::Export(ids)))).into();
    let import = ctx.outline(s.import_button, Some(wrap(Msg::Pick))).into();
    column![
        ctx.caps(s.your_numbers),
        gap(6.0),
        hline(p.dv, 2.0),
        row(s.export_numbers, s.export_numbers_hint, export),
        hline(p.dvs, 1.0),
        row(s.import_numbers, s.import_hint, import),
        hline(p.dvs, 1.0),
    ]
    .align_x(ctx.start())
    .into()
}
