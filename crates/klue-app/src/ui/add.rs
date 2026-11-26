//! The Add wizard: the carrier, the kind of line, the number and password the carrier's app
//! signs in with, then the owner and a nickname. An Orange account can hold a mobile line
//! and home internet; once one is signed in, Klue looks for the other and offers to add it
//! too.

use iced::{
    Alignment, Background, Color, Length, Task,
    font::Weight,
    widget::{Space, button, column, container, operation, stack, text_input},
};
use klue_core::{Carrier, FetchError, LineType, Usage, carriers, format};

use super::{
    Ctx, El, gap, tile,
    modals::{self, Modal, dialog, label, notice, readout},
    window::field,
};
use crate::{
    app::{App, Message},
    assets::{self, Icon},
    i18n,
    product::Product,
    theme,
};

/// Fields focused as their step opens.
const NUMBER: &str = "add-number";
const PASSWORD: &str = "add-password";
const NICKNAME: &str = "add-nickname";

pub struct Wizard {
    step: Step,
    carrier: Carrier,
    product: Product,
    number: String,
    password: String,
    show_password: bool,
    nickname: String,
    phase: Phase,
    others: Others,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    Carrier,
    Line,
    SignIn,
    Finish,
}

const STEPS: [Step; 4] = [Step::Carrier, Step::Line, Step::SignIn, Step::Finish];
/// The steps' least height, so the dialog doesn't jump from one to the next.
const STEP_HEIGHT: f32 = 300.0;

#[derive(Debug, Clone)]
enum Phase {
    Form,
    Verifying,
    Verified(Box<Usage>),
    Error(FetchError),
}

/// The account's other lines.
enum Others {
    /// Not looked for: a carrier with one line per account, or not signed in yet.
    Idle,
    Looking,
    /// Each with whether to add it too.
    Found(Vec<(LineType, Box<Usage>, bool)>),
}

#[derive(Debug, Clone)]
pub enum Msg {
    Carrier(Carrier),
    Product(Product),
    Back,
    Number(String),
    Password(String),
    ShowPassword,
    Nickname(String),
    /// Sign in, or add once signed in.
    Submit,
    /// How signing in with this number went.
    Verified(String, Result<Box<Usage>, FetchError>),
    /// The other lines found for the account signed in with this number.
    Others(String, Vec<(LineType, Box<Usage>)>),
    ToggleOther(usize),
}

impl Wizard {
    /// From the first step, or from the kind of line when the carrier is known.
    pub fn new(carrier: Option<Carrier>) -> Self {
        let known = carrier.filter(|c| c.is_supported());
        let carrier = known.unwrap_or(Carrier::We);
        Self {
            step: if known.is_some() { Step::Line } else { Step::Carrier },
            carrier,
            product: Product::of(carrier)[0],
            number: String::new(),
            password: String::new(),
            show_password: false,
            nickname: String::new(),
            phase: Phase::Form,
            others: Others::Idle,
        }
    }

    fn line(&self) -> LineType {
        self.product.line().unwrap_or(LineType::Mobile)
    }

    fn busy(&self) -> bool {
        matches!(self.phase, Phase::Verifying)
    }

    /// Back to the sign-in, forgetting what it found.
    fn sign_out(&mut self) {
        self.phase = Phase::Form;
        self.others = Others::Idle;
    }
}

fn wrap(msg: Msg) -> Message {
    Message::Modal(modals::Msg::Add(msg))
}

pub fn update(app: &mut App, msg: Msg) -> Task<Message> {
    let step = |app: &App| match &app.modal {
        Modal::Add(w) => Some(w.step),
        _ => None,
    };
    let before = step(app);
    let task = step_update(app, msg);
    // A new step starts at the top of the dialog.
    if step(app).is_some_and(|now| Some(now) != before) { Task::batch([modals::top(), task]) } else { task }
}

fn step_update(app: &mut App, msg: Msg) -> Task<Message> {
    let shared = app.shared.clone();
    let Modal::Add(w) = &mut app.modal else { return Task::none() };
    match msg {
        Msg::Carrier(c) if c.is_supported() && !w.busy() => {
            if c != w.carrier {
                w.carrier = c;
                w.product = Product::of(c)[0];
                w.number.clear();
                w.password.clear();
                w.sign_out();
            }
            w.step = Step::Line;
        }
        Msg::Product(p) if p.line().is_some() && !w.busy() => {
            if p != w.product {
                w.product = p;
                w.sign_out();
            }
            w.step = Step::SignIn;
            return operation::focus(NUMBER);
        }
        Msg::Back if !w.busy() => {
            w.step = match w.step {
                Step::Carrier | Step::Line => Step::Carrier,
                Step::SignIn => Step::Line,
                Step::Finish => {
                    w.sign_out();
                    Step::SignIn
                }
            };
        }
        Msg::Number(n) if w.step == Step::SignIn && !w.busy() => {
            w.number = n;
            if matches!(w.phase, Phase::Error(_)) {
                w.phase = Phase::Form;
            }
        }
        Msg::Password(p) if w.step == Step::SignIn && !w.busy() => {
            w.password = p;
            if matches!(w.phase, Phase::Error(_)) {
                w.phase = Phase::Form;
            }
        }
        Msg::ShowPassword => w.show_password = !w.show_password,
        Msg::Nickname(n) => w.nickname = n,
        Msg::Submit => match (w.step, &w.phase) {
            (Step::SignIn, Phase::Form | Phase::Error(_)) => {
                if w.number.trim().is_empty() || w.password.is_empty() {
                    return Task::none();
                }
                w.phase = Phase::Verifying;
                let (carrier, line, number, password) = (w.carrier, w.line(), w.number.clone(), w.password.clone());
                return Task::perform(
                    async move {
                        let result = shared.verify(carrier, line, &number, &password).await;
                        (number, result.map(Box::new))
                    },
                    |(number, result)| wrap(Msg::Verified(number, result)),
                );
            }
            (Step::Finish, Phase::Verified(usage)) => {
                let (carrier, line, number, password, nickname) = (w.carrier, w.line(), w.number.clone(), w.password.clone(), w.nickname.clone());
                let usage = (**usage).clone();
                let others: Vec<(LineType, Usage)> = match &w.others {
                    Others::Found(list) => list.iter().filter(|(_, _, add)| *add).map(|(l, u, _)| (*l, (**u).clone())).collect(),
                    _ => Vec::new(),
                };
                match shared.add_account(carrier, line, &number, &password, Some(nickname), usage) {
                    Ok(id) => {
                        for (other, usage) in others {
                            if let Err(e) = shared.add_account(carrier, other, &number, &password, None, usage) {
                                tracing::warn!("couldn't add the account's other line: {e}");
                            }
                        }
                        app.modal = Modal::None;
                        app.snap = shared.snapshot();
                        return Task::done(Message::SelectAccount(id));
                    }
                    Err(e) => {
                        w.sign_out();
                        w.phase = Phase::Error(e);
                        w.step = Step::SignIn;
                    }
                }
            }
            _ => {}
        },
        Msg::Verified(number, result) => {
            // A sign-in from before the wizard was closed and opened again doesn't count.
            if !w.busy() || number != w.number {
                return Task::none();
            }
            match result {
                Ok(usage) => {
                    w.phase = Phase::Verified(usage);
                    w.step = Step::Finish;
                    // Look for the account's other line while the user names this one.
                    let lookup = if w.carrier == Carrier::Orange {
                        w.others = Others::Looking;
                        let (carrier, line, number, password) = (w.carrier, w.line(), w.number.clone(), w.password.clone());
                        Task::perform(
                            async move {
                                let found = shared.other_lines(carrier, line, &number, &password).await;
                                (number, found.into_iter().map(|(l, u)| (l, Box::new(u))).collect())
                            },
                            |(number, found)| wrap(Msg::Others(number, found)),
                        )
                    } else {
                        Task::none()
                    };
                    return Task::batch([operation::focus(NICKNAME), lookup]);
                }
                Err(e) => {
                    let retype = e == FetchError::BadCredentials;
                    w.phase = Phase::Error(e);
                    if retype {
                        return operation::focus(PASSWORD);
                    }
                }
            }
        }
        // Only for the sign-in still shown.
        Msg::Others(number, found) if number == w.number && matches!(w.others, Others::Looking) && w.step == Step::Finish => {
            w.others = Others::Found(found.into_iter().map(|(l, u)| (l, u, true)).collect());
        }
        Msg::ToggleOther(i) => {
            if let Others::Found(list) = &mut w.others
                && let Some(other) = list.get_mut(i)
            {
                other.2 = !other.2;
            }
        }
        _ => {}
    }
    Task::none()
}

pub fn view<'a>(ctx: &Ctx<'a>, w: &'a Wizard) -> El<'a> {
    let s = ctx.s;
    let app_name = i18n::carrier_app(w.carrier, ctx.lang);
    let (title, body, foot) = match w.step {
        Step::Carrier => (s.pick_carrier.to_string(), carriers(ctx), foot(ctx, w, None)),
        Step::Line => (s.pick_line.to_string(), lines(ctx, w), foot(ctx, w, None)),
        Step::SignIn => (i18n::sign_in_title(app_name, ctx.lang), sign_in(ctx, w), sign_in_foot(ctx, w)),
        Step::Finish => (s.ready_to_add.to_string(), finish(ctx, w), finish_foot(ctx, w)),
    };
    let kicker = match w.step {
        Step::Carrier => s.new_number.to_string(),
        _ => format!("{} · {}", s.new_number, i18n::carrier_name(w.carrier, ctx.lang)),
    };
    // The stack is as tall as its tallest layer.
    let body = column![stepper(ctx, w.step), stack![Space::new().width(Length::Fill).height(STEP_HEIGHT), body]].spacing(22);
    dialog(ctx, 580.0, kicker, title, Some(Message::CloseModal), body.into(), foot)
}

/// Numbered steps joined by lines; the ones done get a tick.
fn stepper<'a>(ctx: &Ctx<'a>, at: Step) -> El<'a> {
    let p = ctx.p;
    let mut items: Vec<El<'a>> = Vec::new();
    for (i, (step, name)) in STEPS.into_iter().zip(ctx.s.wizard_steps).enumerate() {
        if i > 0 {
            items.push(container(super::hline(if step <= at { p.ac } else { p.dv }, 2.0)).width(Length::Fill).padding([0, 10]).into());
        }
        let reached = step <= at;
        let mark: El<'a> = if step < at {
            ctx.icon(Icon::Check, 12.0, p.on)
        } else {
            ctx.num((i + 1).to_string(), 12.0, Weight::ExtraBold, if reached { p.on } else { p.mu }).into()
        };
        let dot = container(mark).center(24).style(move |_| {
            let mut st = theme::fill(if reached { p.ac } else { Color::TRANSPARENT }, 12.0);
            st.border = theme::border(if reached { p.ac } else { p.dv }, 2.0, 12.0);
            st
        });
        let weight = if step == at { Weight::ExtraBold } else { Weight::Semibold };
        items.push(ctx.row(vec![dot.into(), ctx.text(name, 12.5, weight, if reached { p.tx } else { p.mu }).into()]).spacing(8).into());
    }
    ctx.row(items).width(Length::Fill).into()
}

/// A big choice: highlighted when picked or hovered; faded when it can't be picked.
fn card<'a>(ctx: &Ctx<'a>, content: El<'a>, on: bool, press: Option<Message>) -> button::Button<'a, Message> {
    let p = ctx.p;
    let enabled = press.is_some();
    button(content).width(Length::Fill).padding([14, 16]).on_press_maybe(press).style(move |_, status| {
        let hovered = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: (hovered || on).then_some(Background::Color(p.sf)),
            border: theme::border(if hovered || on { p.ac } else { p.dv }, 2.0, 14.0),
            text_color: p.tx,
            ..Default::default()
        }
    })
}

/// Two to a row.
fn grid<'a>(ctx: &Ctx<'a>, cells: Vec<El<'a>>) -> El<'a> {
    let mut cells = cells.into_iter();
    let mut rows = column![].spacing(12);
    while let Some(first) = cells.next() {
        let second = cells.next().unwrap_or_else(|| Space::new().width(Length::Fill).into());
        rows = rows.push(ctx.row(vec![container(first).width(Length::FillPortion(1)).into(), container(second).width(Length::FillPortion(1)).into()]).spacing(12));
    }
    rows.into()
}

// ---------------------------------------------------------------------------------------
// Steps

fn carriers<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let cells = Carrier::ALL
        .into_iter()
        .map(|c| {
            let ready: Vec<&str> = Product::of(c).iter().filter(|x| x.line().is_some()).map(|x| i18n::product(*x, ctx.lang).0).collect();
            let open = c.is_supported() && !ready.is_empty();
            let what = if open { ready.join(if ctx.rtl() { "، " } else { ", " }) } else { ctx.s.coming_soon.to_string() };
            let t = assets::tile(c);
            let content = column![
                tile(c, 48.0, t.rail, 13.0, !open),
                gap(6.0),
                ctx.text(i18n::carrier_name(c, ctx.lang), 16.0, Weight::ExtraBold, if open { p.tx } else { p.mu }),
                ctx.muted(what, 12.5),
            ]
            .spacing(2)
            .width(Length::Fill)
            .align_x(ctx.start());
            card(ctx, content.into(), false, open.then(|| wrap(Msg::Carrier(c)))).height(128).into()
        })
        .collect();
    grid(ctx, cells)
}

fn lines<'a>(ctx: &Ctx<'a>, w: &'a Wizard) -> El<'a> {
    let p = ctx.p;
    let cells = Product::of(w.carrier)
        .iter()
        .map(|&product| {
            let open = product.line().is_some();
            let (name, about) = i18n::product(product, ctx.lang);
            let glyph = container(ctx.icon(product.icon(), 20.0, if open { p.ac_tx } else { p.mu }))
                .center(42)
                .style(move |_| theme::fill(if open { theme::fade(p.ac, 0.22) } else { p.sf }, 11.0));
            let mut head = vec![ctx.text(name, 14.0, Weight::ExtraBold, if open { p.tx } else { p.mu }).into()];
            if !open {
                head.push(container(ctx.text(ctx.s.soon, 10.5, Weight::Semibold, p.mu)).padding([1, 7]).style(theme::bg_line(Color::TRANSPARENT, p.dv, 8.0)).into());
            }
            let text = column![ctx.row(head).spacing(8), ctx.muted(about, 12.0)].spacing(3).width(Length::Fill).align_x(ctx.start());
            let content = ctx.row(vec![glyph.into(), text.into()]).spacing(12).align_y(Alignment::Center);
            card(ctx, content.into(), open && w.product == product && w.step > Step::Line, open.then(|| wrap(Msg::Product(product)))).height(82).into()
        })
        .collect();
    grid(ctx, cells)
}

fn sign_in<'a>(ctx: &Ctx<'a>, w: &'a Wizard) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let app_name = i18n::carrier_app(w.carrier, ctx.lang);
    let busy = w.busy();
    let error = match &w.phase {
        Phase::Error(e) => Some(e),
        _ => None,
    };

    // Like the carrier's own sign-in screen: its logo, its app, then the two fields.
    let t = assets::tile(w.carrier);
    let head = column![
        tile(w.carrier, 52.0, t.rail * 1.05, 15.0, false),
        gap(4.0),
        ctx.bold(app_name, 18.0),
        ctx.muted(format!("{} · {}", i18n::carrier_name(w.carrier, ctx.lang), i18n::product(w.product, ctx.lang).0), 12.5),
    ]
    .spacing(2)
    .align_x(Alignment::Center)
    .width(Length::Fill);

    let (number_label, placeholder, hint) = match w.product {
        Product::WeHomeInternet => (s.landline_number.to_string(), "02 2345 6789", Some(s.area_code_hint.to_string())),
        Product::OrHomeInternet => (i18n::app_number(app_name, ctx.lang), "012 3456 7890", Some(i18n::linked_landline_hint(app_name, ctx.lang))),
        Product::OrMobile => (s.mobile_number.to_string(), "012 3456 7890", None),
        _ => (s.mobile_number.to_string(), "010 1234 5678", None),
    };
    let mut number = field(ctx, placeholder, &w.number, |t| wrap(Msg::Number(t)), false).font(assets::numbers(Weight::Normal)).on_submit(wrap(Msg::Submit));
    if busy {
        number = text_input(placeholder, &w.number).padding([9, 10]).size(14).font(assets::numbers(Weight::Normal)).style(theme::field(p, false));
    }
    let mut number_col = column![label(ctx, &number_label), gap(6.0), number.id(NUMBER)].align_x(ctx.start());
    if let Some(hint) = hint {
        number_col = number_col.push(gap(6.0)).push(ctx.muted(hint, 12.0));
    }

    let mut password = text_input("", &w.password)
        .id(PASSWORD)
        .secure(!w.show_password)
        .padding([9, 10])
        .size(14)
        .font(assets::numbers(Weight::Normal))
        .style(theme::field(p, error == Some(&FetchError::BadCredentials)))
        .on_submit(wrap(Msg::Submit));
    if !busy {
        password = password.on_input(|t| wrap(Msg::Password(t)));
    }
    let eye = button(container(ctx.icon(if w.show_password { Icon::EyeOff } else { Icon::Eye }, 16.0, p.mu)).center(Length::Fill))
        .width(38)
        .height(38)
        .padding(0)
        .style(theme::ghost(p.mu, p.sf, 9.0))
        .on_press(wrap(Msg::ShowPassword));
    let mut password_col = column![
        label(ctx, s.password),
        gap(6.0),
        ctx.row(vec![password.into(), eye.into()]).spacing(6),
        gap(6.0),
        ctx.muted(i18n::same_password(app_name, ctx.lang), 12.0)
    ]
    .align_x(ctx.start());
    if busy {
        password_col = password_col.push(gap(10.0)).push(ctx.meter(0.35, 3.0, p.ac));
    }

    // What went wrong goes first, where it's seen without scrolling.
    let mut body = column![].spacing(18);
    if let Some(e) = error {
        body = body.push(notice(ctx, e, w.carrier));
    }
    body.push(head).push(number_col).push(password_col).into()
}

fn finish<'a>(ctx: &Ctx<'a>, w: &'a Wizard) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let Phase::Verified(usage) = &w.phase else { return Space::new().into() };
    let sign_in = format::normalize(&w.number, w.carrier.sign_in_line(w.line())).unwrap_or_else(|_| w.number.clone());
    let own = carriers::line_number(w.carrier, w.line(), &sign_in, usage).unwrap_or(sign_in.clone());

    let owner = usage.owner_name.clone().unwrap_or_else(|| s.owner_unknown.into());
    let mut body = column![
        summary(ctx, w.carrier, w.product, &own, usage),
        column![ctx.row(vec![label(ctx, s.owner_name), ctx.text(s.from_account, 11.0, Weight::Semibold, p.ac_tx).into()]), gap(6.0), readout(ctx, owner, true, true)],
        column![
            label(ctx, s.nick_optional),
            gap(6.0),
            field(ctx, s.nick_ph, &w.nickname, |t| wrap(Msg::Nickname(t)), false).on_submit(wrap(Msg::Submit)).id(NICKNAME)
        ],
    ]
    .spacing(16);

    match &w.others {
        Others::Idle => {}
        Others::Looking => {
            body = body.push(
                column![ctx.muted(s.looking_for_lines, 12.5), container(ctx.meter(0.35, 3.0, p.ac)).width(160)].spacing(8).align_x(ctx.start()),
            );
        }
        Others::Found(list) => {
            for (i, (line, other, add)) in list.iter().enumerate() {
                let number = carriers::line_number(w.carrier, *line, &sign_in, other).unwrap_or_default();
                let mut about = vec![format::pretty_number(&number)];
                about.extend(other.package_name.clone());
                if let Some(main) = other.ordered(None).first() {
                    about.push(i18n::left(&format!("{} {}", format::amount(main.remaining, main.unit), i18n::unit_name(main.unit, ctx.lang)), ctx.lang));
                }
                let text = column![ctx.bold(i18n::also_has(*line, ctx.lang), 14.0), ctx.muted(about.join(" · "), 12.5)].spacing(3).width(Length::Fill).align_x(ctx.start());
                let glyph = Product::for_line(w.carrier, *line).map_or(Icon::Mobile, Product::icon);
                let row = ctx.row(vec![ctx.icon(glyph, 20.0, p.ac_tx), text.into(), ctx.toggle(*add, wrap(Msg::ToggleOther(i)))]).spacing(14);
                body = body.push(container(row).padding([12, 14]).style(theme::bg_line(p.sf, if *add { p.ac } else { p.dv }, 12.0)));
            }
        }
    }
    body.into()
}

/// The line just signed in to: what it is, its number, its package and what's left.
fn summary<'a>(ctx: &Ctx<'a>, carrier: Carrier, product: Product, number: &str, usage: &Usage) -> El<'a> {
    let p = ctx.p;
    let t = assets::tile(carrier);
    let mut about: Vec<String> = usage.package_in(ctx.lang).map(String::from).into_iter().collect();
    if let Some(main) = usage.ordered(None).first() {
        about.push(i18n::left(&format!("{} {}", format::amount(main.remaining, main.unit), i18n::unit_name(main.unit, ctx.lang)), ctx.lang));
    }
    let text = column![
        ctx.caps(&format!("{} · {}", i18n::carrier_name(carrier, ctx.lang), i18n::product(product, ctx.lang).0)),
        ctx.num(format::pretty_number(number), 17.0, Weight::ExtraBold, p.tx),
        ctx.muted(about.join(" · "), 12.5),
    ]
    .spacing(2)
    .width(Length::Fill)
    .align_x(ctx.start());
    let check = container(ctx.icon(Icon::Check, 14.0, p.on)).center(26).style(theme::bg(p.ac, 13.0));
    container(ctx.row(vec![tile(carrier, 46.0, t.rail, 13.0, false), text.into(), check.into()]).spacing(14))
        .padding([12, 14])
        .style(theme::bg_line(p.sf, p.dvs, 14.0))
        .into()
}

// ---------------------------------------------------------------------------------------
// Footers

/// Back (after the first step) and Cancel, with `primary` last.
fn foot<'a>(ctx: &Ctx<'a>, w: &'a Wizard, primary: Option<El<'a>>) -> El<'a> {
    let s = ctx.s;
    let mut items: Vec<El<'a>> = Vec::new();
    if w.step > Step::Carrier {
        items.push(ctx.outline(s.back, (!w.busy()).then(|| wrap(Msg::Back))).into());
    } else {
        // A number exported on another computer comes in from here too.
        items.push(ctx.link(s.import_file, Message::Modal(modals::Msg::Transfer(super::transfer::Msg::Pick))).into());
    }
    items.push(super::fill_x());
    match primary {
        Some(primary) => items.push(primary),
        None => items.push(ctx.outline(s.cancel, Some(Message::CloseModal)).into()),
    }
    ctx.row(items).spacing(8).into()
}

fn sign_in_foot<'a>(ctx: &Ctx<'a>, w: &'a Wizard) -> El<'a> {
    let s = ctx.s;
    let ready = !w.number.trim().is_empty() && !w.password.is_empty();
    let (label, press) = match &w.phase {
        Phase::Verifying => (s.verifying, None),
        Phase::Error(_) => (s.try_again, ready.then(|| wrap(Msg::Submit))),
        _ => (s.sign_in, ready.then(|| wrap(Msg::Submit))),
    };
    let status: El<'a> = if w.busy() { ctx.muted(i18n::signing_in(w.carrier, ctx.lang), 12.5).into() } else { Space::new().into() };
    // Fixed width, so it doesn't change size as its label does.
    let button = button(container(ctx.text(label, 14.0, Weight::ExtraBold, ctx.p.on)).center_x(Length::Fill))
        .width(150)
        .padding([9, 14])
        .style(theme::primary(ctx.p))
        .on_press_maybe(press);
    let primary = ctx.row(vec![status, button.into()]).spacing(12).into();
    foot(ctx, w, Some(primary))
}

fn finish_foot<'a>(ctx: &Ctx<'a>, w: &'a Wizard) -> El<'a> {
    let s = ctx.s;
    let both = matches!(&w.others, Others::Found(list) if list.iter().any(|(_, _, add)| *add));
    let status = ctx.text(format!("✓ {}", s.signed_in), 12.5, Weight::Semibold, ctx.p.ac_tx);
    let primary = ctx.row(vec![status.into(), ctx.primary(if both { s.add_both } else { s.add_number }, Some(wrap(Msg::Submit))).into()]).spacing(12).into();
    foot(ctx, w, Some(primary))
}
