//! The main window: title bar, carrier rail, number tabs and the dashboard, with dialogs
//! on top.

use iced::{
    Alignment, Color, Length,
    alignment::Horizontal,
    font::Weight,
    mouse,
    widget::{Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text_input},
    window::Direction,
};
use iced::alignment::Vertical;
use klue_core::{Carrier, format, store::Account};

use super::{Ctx, El, dashboard, empty, hline, modals, tile, vline};
use crate::{
    app::{App, DragItem, Message, ModalKind},
    assets::Icon,
    i18n,
    theme,
};

pub fn view(app: &App) -> El<'_> {
    let shown = app.shown_carrier();
    let ctx = Ctx::new(app, shown.unwrap_or(Carrier::We), false);
    let p = ctx.p;

    let content: El<'_> = match app.current() {
        Some(account) => dashboard::view(&ctx, account),
        None => empty::view(&ctx),
    };
    let main = column![tabs(&ctx), content].width(Length::Fill).height(Length::Fill);
    let body = ctx.row(vec![rail(&ctx), vline(p.chdv, 2.0), main.into()]).height(Length::Fill).align_y(Alignment::Start);
    // No background; the window is cleared to `p.bg`.
    let page = container(column![titlebar(&ctx), hline(p.chdv, 2.0), body]).width(Length::Fill).height(Length::Fill);

    let mut layers: Vec<El<'_>> = vec![page.into()];
    if let Some(dialog) = modals::view(&ctx) {
        layers.push(overlay(dialog, Message::CloseModal));
    }
    if app.update.dialog()
        && let Some(dialog) = modals::update_dialog(&ctx)
    {
        layers.push(overlay(dialog, Message::UpdateLater));
    }
    // On top of everything; only Accept or Quit closes it.
    if app.disclaimer_due() {
        layers.push(overlay(modals::disclaimer(&ctx), Message::Done));
    }
    if let Some(ask) = &app.captcha {
        layers.push(overlay(modals::captcha(&ctx, ask), Message::CaptchaCancel));
    }
    if !cfg!(target_os = "macos") {
        layers.push(resize_edges());
    }
    stack(layers).width(Length::Fill).height(Length::Fill).into()
}

/// Dims the window behind `dialog`; clicking the dim area sends `dismiss`.
fn overlay<'a>(dialog: El<'a>, dismiss: Message) -> El<'a> {
    let dim = mouse_area(container(Space::new()).width(Length::Fill).height(Length::Fill).style(|_| theme::fill(Color::from_rgba8(10, 9, 9, 0.52), 0.0)))
        .on_press(dismiss);
    let centred = container(opaque(dialog)).center(Length::Fill).padding(24);
    opaque(stack([dim.into(), centred.into()]))
}

/// Invisible resize handles along the edges (there's no native frame).
fn resize_edges<'a>() -> El<'a> {
    const T: f32 = 5.0;
    let edge = |w: Length, h: Length, dir: Direction, cursor: mouse::Interaction| -> El<'a> {
        mouse_area(Space::new().width(w).height(h)).on_press(Message::DragResize(dir)).interaction(cursor).into()
    };
    let ns = mouse::Interaction::ResizingVertically;
    let ew = mouse::Interaction::ResizingHorizontally;
    let nwse = mouse::Interaction::ResizingDiagonallyDown;
    let nesw = mouse::Interaction::ResizingDiagonallyUp;
    let top = row![
        edge(T.into(), T.into(), Direction::NorthWest, nwse),
        edge(Length::Fill, T.into(), Direction::North, ns),
        edge(T.into(), T.into(), Direction::NorthEast, nesw)
    ];
    let middle = row![
        edge(T.into(), Length::Fill, Direction::West, ew),
        Space::new().width(Length::Fill).height(Length::Fill),
        edge(T.into(), Length::Fill, Direction::East, ew)
    ];
    let bottom = row![
        edge(T.into(), T.into(), Direction::SouthWest, nesw),
        edge(Length::Fill, T.into(), Direction::South, ns),
        edge(T.into(), T.into(), Direction::SouthEast, nwse)
    ];
    column![top, middle.height(Length::Fill), bottom].into()
}

// ---------------------------------------------------------------------------------------
// Title bar

fn titlebar<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let all = ctx.app.snap.refreshing_all;
    // The shown number's own last update: numbers refresh on their own schedules.
    let current = ctx.app.current();
    let syncing = all || current.is_some_and(|a| ctx.app.snap.is_refreshing(&a.id));
    let synced = if syncing { s.syncing.to_string() } else { i18n::synced(current.and_then(Account::updated_at), ctx.now, ctx.lang) };
    // Refresh all; each number also has its own Refresh.
    let has_numbers = !ctx.app.snap.data.accounts.is_empty();
    let refresh_all = ctx.tip(
        button(container(ctx.icon(Icon::Refresh, 14.0, if all { p.chm } else { p.cht })).center(Length::Fill))
            .width(28)
            .height(28)
            .padding(0)
            .style(theme::chrome_button(p))
            .on_press_maybe((!all && has_numbers).then_some(Message::RefreshAll)),
        s.refresh_all,
    );

    let brand = ctx
        .row(vec![super::logo(16.0), ctx.text("Klue", 14.0, Weight::ExtraBold, p.cht).into(), ctx.text(synced, 12.0, Weight::Normal, p.chm).into(), refresh_all])
        .spacing(10)
        .align_y(Alignment::Center);
    // Drag the empty title bar to move the window.
    let handle = mouse_area(container(brand).width(Length::Fill).height(Length::Fill).align_x(ctx.start()).align_y(Alignment::Center))
        .on_press(Message::Drag)
        .on_double_click(Message::ToggleMaximize);
    let middle = ctx.row(vec![handle.into()]).width(Length::Fill).height(Length::Fill);

    // --debug: console button next to the window buttons.
    let console: El<'a> = if ctx.app.debug {
        ctx.tip(
            button(container(ctx.icon(Icon::Terminal, 15.0, p.cht)).center(Length::Fill)).width(46).height(Length::Fill).padding(0).style(theme::window_button(p, false)).on_press(Message::OpenConsole),
            ctx.s.debug_console,
        )
    } else {
        Space::new().into()
    };

    // macOS has its traffic lights at the top left; elsewhere the window buttons go on the
    // right, in both languages.
    let bar = if cfg!(target_os = "macos") {
        row![Space::new().width(78), middle, console, Space::new().width(12)]
    } else {
        let win = |icon: Icon, msg: Message, close: bool| -> El<'a> {
            button(container(ctx.icon(icon, 14.0, p.cht)).center(Length::Fill))
                .width(46)
                .height(Length::Fill)
                .padding(0)
                .style(theme::window_button(p, close))
                .on_press(msg)
                .into()
        };
        row![
            Space::new().width(16),
            middle,
            Space::new().width(12),
            console,
            win(Icon::WinMin, Message::Minimize, false),
            win(Icon::WinMax, Message::ToggleMaximize, false),
            win(Icon::WinClose, Message::CloseMain, true)
        ]
    };
    container(bar.height(40).align_y(Alignment::Center)).height(40).style(move |_| theme::fill(p.ch, 0.0)).into()
}

// ---------------------------------------------------------------------------------------
// Carrier rail

fn rail<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let data = &ctx.app.snap.data;
    // Only carriers with numbers; the rest are reached through Add.
    let shown = ctx.app.shown_carrier();
    let carriers: Vec<Carrier> = data.carriers().into_iter().filter(|carrier| data.accounts_for(*carrier).next().is_some()).collect();
    let items: Vec<El<'a>> = carriers
        .iter()
        .enumerate()
        .map(|(index, &carrier)| {
            let on = Some(carrier) == shown;
            let count = data.accounts_for(carrier).count();
            let t = crate::assets::tile(carrier);
            let me = DragItem::Carrier(carrier);
            let (dragged, drop) = drag_marks(ctx, &me, index, |item| match item {
                DragItem::Carrier(c) => carriers.iter().position(|x| x == c),
                DragItem::Line(_) => None,
            });
            let tile_el = tile(carrier, 46.0, t.rail, 13.0, dragged);
            let count_badge = badge(ctx, &count.to_string(), 11.0);
            let tile_with_badge = stack([
                container(tile_el).padding([8, 12]).into(),
                container(count_badge).width(Length::Fill).align_x(if ctx.rtl() { Horizontal::Left } else { Horizontal::Right }).into(),
            ])
            .width(70);
            let label = ctx.text(i18n::carrier_name(carrier, ctx.lang), 12.0, Weight::Semibold, if on { p.cht } else { p.chm });
            let inner = column![tile_with_badge, label].align_x(Alignment::Center).spacing(2).width(Length::Fill);
            let indicator = container(Space::new()).width(4).height(52).style(move |_| theme::fill(if on { p.ch_bar } else { Color::TRANSPARENT }, 2.0));
            // Pressing picks it on release, or drags it to another place.
            let grip = mouse_area(inner).on_press(Message::DragStart(me.clone())).on_enter(Message::DragOver(me));
            let item = button(grip).width(Length::Fill).padding([6, 0]).style(theme::plain(p.ch_act, on.then_some(p.ch_act), 14.0)).on_press(Message::SelectCarrier(carrier));
            let entry: El<'a> = ctx.row(vec![indicator.into(), container(item).padding([0, 8]).width(Length::Fill).into()]).into();
            match drop {
                // A bar above or below, on top of the item so nothing moves under the pointer.
                Some(after) => {
                    let bar = container(container(Space::new()).width(Length::Fill).height(3).style(move |_| theme::fill(p.ch_bar, 2.0))).padding([0, 14]);
                    stack([entry, container(bar).width(Length::Fill).height(Length::Fill).align_y(if after { Vertical::Bottom } else { Vertical::Top }).into()]).into()
                }
                None => entry,
            }
        })
        .collect();

    // `count` puts a badge on the icon.
    let tool = |icon: Icon, label: &'a str, msg: Message, count: Option<usize>| -> El<'a> {
        let glyph: El<'a> = match count {
            Some(n) => stack([
                container(ctx.icon(icon, 18.0, p.chm)).center_x(Length::Fill).padding([3, 0]).into(),
                container(badge(ctx, &n.to_string(), 10.0)).width(Length::Fill).align_x(Horizontal::Center).padding(ctx.pad(0.0, 0.0, 0.0, 30.0)).into(),
            ])
            .width(Length::Fill)
            .into(),
            None => ctx.icon(icon, 18.0, p.chm),
        };
        let content = column![glyph, ctx.text(label, 11.0, Weight::Normal, p.chm)].spacing(4).align_x(Alignment::Center).width(Length::Fill);
        container(button(content).width(Length::Fill).padding([10, 0]).style(theme::plain(p.ch_act, None, 12.0)).on_press(msg)).padding([0, 8]).into()
    };
    let archived = data.archived().count();
    let mut col = column(items).spacing(4).push(Space::new().height(Length::Fill)).push(tool(Icon::Plus, s.add, Message::OpenModal(ModalKind::Add(None)), None));
    // Only once something is archived.
    if archived > 0 {
        col = col.push(tool(Icon::Archive, s.archived_rail, Message::OpenModal(ModalKind::Archived), Some(archived)));
    }
    let col = col.push(tool(Icon::Sliders, s.settings, Message::OpenModal(ModalKind::Settings), None)).padding([8, 0]).width(92).height(Length::Fill);
    container(col).height(Length::Fill).style(move |_| theme::fill(p.ch, 0.0)).into()
}

fn badge<'a>(ctx: &Ctx<'a>, label: &str, size: f32) -> El<'a> {
    let p = ctx.p;
    container(ctx.num(label.to_string(), size, Weight::ExtraBold, p.ch)).padding([1, 6]).style(move |_| theme::fill(p.cht, 10.0)).into()
}

// ---------------------------------------------------------------------------------------
// Number tabs

fn tabs<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let app = ctx.app;
    let current = app.current().map(|a| a.id.clone());
    let shown = app.shown_carrier();
    let accounts: Vec<&'a Account> = shown.into_iter().flat_map(|carrier| app.snap.data.accounts_for(carrier)).collect();
    let mut items: Vec<El<'a>> = accounts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            let on = current.as_deref() == Some(account.id.as_str());
            let icon = super::line_icon(account.line);
            let me = DragItem::Line(account.id.clone());
            let (dragged, drop) = drag_marks(ctx, &me, index, |item| match item {
                DragItem::Line(id) => accounts.iter().position(|a| &a.id == id),
                DragItem::Carrier(_) => None,
            });
            let on = on && !dragged;
            let fg = if on { p.tx } else if dragged { theme::fade(p.mu, 0.55) } else { p.mu };
            let text_col = column![
                ctx.text(account.display_name(), 14.0, Weight::ExtraBold, fg),
                ctx.num(format::pretty_number(&account.number), 11.5, Weight::Normal, fg)
            ]
            .spacing(0)
            .align_x(ctx.start());
            let mut parts: Vec<El<'a>> = vec![ctx.icon(icon, 16.0, fg), text_col.into()];
            if account.last_error.is_some() {
                parts.push(container(Space::new()).width(8).height(8).style(move |_| theme::fill(p.danger, 4.0)).into());
            }
            let accent = container(Space::new()).width(Length::Fill).height(3).style(move |_| theme::fill(if on { p.ac } else { Color::TRANSPARENT }, 2.0));
            let label = container(ctx.row(parts).spacing(10)).padding([0, 18]).height(45).align_y(Alignment::Center);
            let tab = column![container(accent).padding([0, 14]), label];
            // Pressing picks it on release, or drags it to another place.
            let grip = mouse_area(tab).on_press(Message::DragStart(me.clone())).on_enter(Message::DragOver(me));
            let tab = button(grip)
                .padding(0)
                .style(move |_, status| {
                    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                    let bg = if on { Some(p.bg) } else if hovered { Some(theme::mix(p.strip, p.bg, 0.5)) } else { None };
                    button::Style {
                        background: bg.map(iced::Background::Color),
                        border: iced::Border { radius: iced::border::Radius { top_left: 12.0, top_right: 12.0, bottom_right: 0.0, bottom_left: 0.0 }, ..Default::default() },
                        ..Default::default()
                    }
                })
                .on_press(Message::SelectAccount(account.id.clone()));
            match drop {
                // A bar at the side it lands on, on top of the tab so nothing moves under the pointer.
                Some(after) => {
                    let bar = container(Space::new()).width(3).height(28).style(move |_| theme::fill(p.ac, 2.0));
                    let side = if after { ctx.end() } else { ctx.start() };
                    stack([tab.into(), container(bar).width(Length::Fill).height(Length::Fill).align_x(side).align_y(Alignment::End).padding([8, 0]).into()]).into()
                }
                None => tab.into(),
            }
        })
        .collect();
    let add = button(ctx.row(vec![ctx.icon(Icon::Plus, 15.0, p.mu), ctx.text(ctx.s.add_number, 13.0, Weight::Normal, p.mu).into()]).spacing(8))
        .padding([8, 12])
        .style(theme::ghost(p.mu, p.bg, 10.0))
        .on_press(Message::OpenModal(ModalKind::Add(shown)));
    items.push(container(add).padding(ctx.pad(0.0, 0.0, 6.0, 4.0)).into());
    let strip = ctx.row(items).spacing(4).align_y(Alignment::End).height(58);
    let scroller = scrollable(container(strip).padding([0, 22])).direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(0).scroller_width(0)));
    container(super::smooth::smooth(scroller)).width(Length::Fill).height(58).style(move |_| theme::fill(p.strip, 0.0)).into()
}

/// While something is dragged: whether `me` is the dragged item (and elsewhere), and whether
/// it would land on `me`, and on which side (`true`: after it, in reading order). `index`
/// finds an item's place in the list.
fn drag_marks(ctx: &Ctx<'_>, me: &DragItem, at: usize, index: impl Fn(&DragItem) -> Option<usize>) -> (bool, Option<bool>) {
    let Some(drag) = &ctx.app.drag else { return (false, None) };
    if drag.over == drag.item {
        return (false, None);
    }
    let dragged = drag.item == *me;
    let drop = (drag.over == *me).then(|| index(&drag.item).is_some_and(|from| from < at));
    (dragged, drop)
}

/// Text field style for dialogs.
pub fn field<'a>(ctx: &Ctx<'a>, placeholder: &str, value: &str, on_input: impl Fn(String) -> Message + 'a, error: bool) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .padding([9, 10])
        .size(14)
        .font(crate::assets::font(ctx.lang, Weight::Normal))
        .style(theme::field(ctx.p, error))
}
