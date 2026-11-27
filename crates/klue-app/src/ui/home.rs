//! The home page: groups of lines from any carrier, each a tab with a grid of cards to take
//! in at a glance. A card opens its line; the last card picks what the group shows.

use std::collections::HashSet;

use iced::{
    Alignment, Background, Color, Length, Task,
    alignment::Vertical,
    font::Weight,
    widget::{Space, button, column, container, mouse_area, scrollable, stack},
};
use klue_core::{
    Bundle, BundleState, format,
    store::{Account, AppData, Card, Group},
};

use super::{
    Ctx, El, gap, hline, tile,
    modals::{self, Modal, dialog, label},
    window::{drag_marks, field},
};
use crate::{
    app::{App, DragItem, Message, ModalKind},
    assets::{self, Icon},
    i18n, theme,
};

/// Focused as the group dialog opens.
pub const GROUP_NAME: &str = "group-name";
const CARD_HEIGHT: f32 = 172.0;
/// A card is at least this wide; the grid takes as many columns as fit.
const CARD_WIDTH: f32 = 290.0;
const GAP: f32 = 16.0;

/// Naming a new group, or renaming or deleting one.
pub struct GroupForm {
    /// `None` for a new group.
    id: Option<String>,
    name: String,
    confirm_delete: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Name(String),
    Save,
    Delete,
    /// Puts a card on a group, or takes it off.
    Toggle(String, Card),
    /// Shows or hides a line's other bundles in the picker.
    Expand(String),
}

/// Picking what a group shows: its id, and the lines whose other bundles are shown.
pub struct LinesPicker {
    pub group: String,
    open: HashSet<String>,
}

impl LinesPicker {
    /// Lines that already have a bundle's card here start open.
    pub fn new(group: String, data: &AppData) -> Self {
        let open = data.group(&group).map(|g| g.cards.into_iter().filter(|c| c.bundle.is_some()).map(|c| c.line).collect()).unwrap_or_default();
        Self { group, open }
    }
}

fn wrap(msg: Msg) -> Message {
    Message::Modal(modals::Msg::Home(msg))
}

impl GroupForm {
    pub fn new(id: Option<String>, data: &AppData) -> Self {
        let s = i18n::strings(data.settings.lang);
        let name = match &id {
            Some(id) => data.group(id).and_then(|g| g.name).unwrap_or_default(),
            None => format!("{} {}", s.group, data.groups().len() + 1),
        };
        Self { id, name, confirm_delete: false }
    }
}

/// What a group shows as: its name, "My lines" for the first until it's named, else "Group 2".
pub fn group_name(group: &Group, index: usize, s: &i18n::Strings) -> String {
    match &group.name {
        Some(name) => name.clone(),
        None if group.id == Group::FIRST => s.my_lines.to_string(),
        None => format!("{} {}", s.group, index + 1),
    }
}

pub fn update(app: &mut App, msg: Msg) -> Task<Message> {
    let shared = app.shared.clone();
    match (&mut app.modal, msg) {
        (_, Msg::Toggle(group, card)) => {
            let on = app.snap.data.group(&group).is_some_and(|g| g.has(&card));
            shared.update(|d| d.set_card(&group, card, !on));
        }
        (Modal::GroupLines(f), Msg::Expand(line)) => {
            if !f.open.remove(&line) {
                f.open.insert(line);
            }
        }
        (Modal::Group(f), Msg::Name(name)) => {
            f.name = name;
            f.confirm_delete = false;
        }
        (Modal::Group(f), Msg::Save) => match f.id.clone() {
            Some(id) => {
                let name = f.name.clone();
                shared.update(|d| d.rename_group(&id, &name));
                app.modal = Modal::None;
            }
            // A new group goes straight on to picking its lines.
            None => {
                let name = Some(f.name.clone());
                let id = shared.update(|d| d.add_group(name));
                app.modal = Modal::None;
                app.home = true;
                app.group = id.clone();
                return Task::done(Message::OpenModal(ModalKind::GroupLines(id)));
            }
        },
        (Modal::Group(f), Msg::Delete) => {
            let Some(id) = f.id.clone() else { return Task::none() };
            if f.confirm_delete {
                shared.update(|d| d.remove_group(&id));
                app.modal = Modal::None;
                app.snap = shared.snapshot();
                app.group = app.snap.data.groups().first().map_or_else(|| Group::FIRST.to_string(), |g| g.id.clone());
            } else {
                f.confirm_delete = true;
            }
        }
        _ => {}
    }
    Task::none()
}

// ---------------------------------------------------------------------------------------
// The page

/// The group tabs, where the number tabs are on a carrier's page.
pub fn strip<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let app = ctx.app;
    let groups = app.snap.data.groups();
    let current = app.shown_group().id;
    let ids: Vec<String> = groups.iter().map(|g| g.id.clone()).collect();
    let mut items: Vec<El<'a>> = groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let me = DragItem::Group(group.id.clone());
            let (dragged, drop) = drag_marks(ctx, &me, index, |item| match item {
                DragItem::Group(id) => ids.iter().position(|g| g == id),
                _ => None,
            });
            let on = group.id == current && !dragged;
            let fg = if on { p.tx } else if dragged { theme::fade(p.mu, 0.55) } else { p.mu };
            let count = app.snap.data.group_line_count(&group.id);
            let text_col = column![
                ctx.text(group_name(group, index, s), 14.0, Weight::ExtraBold, fg),
                ctx.text(i18n::lines_count(count, ctx.lang), 11.5, Weight::Normal, fg)
            ]
            .spacing(0)
            .align_x(ctx.start());
            let accent = container(Space::new()).width(Length::Fill).height(3).style(move |_| theme::fill(if on { p.ac } else { Color::TRANSPARENT }, 2.0));
            let label = container(ctx.row(vec![ctx.icon(Icon::House, 16.0, fg), text_col.into()]).spacing(10)).padding([0, 18]).height(45).align_y(Alignment::Center);
            let tab = column![container(accent).padding([0, 14]), label];
            // Pressing picks it on release, or drags it to another place.
            let grip = mouse_area(tab).on_press(Message::DragStart(me.clone())).on_enter(Message::DragOver(me));
            let tab = button(grip)
                .padding(0)
                .style(move |_, status| {
                    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                    let bg = if on { Some(p.bg) } else if hovered { Some(theme::mix(p.strip, p.bg, 0.5)) } else { None };
                    button::Style {
                        background: bg.map(Background::Color),
                        border: iced::Border { radius: iced::border::Radius { top_left: 12.0, top_right: 12.0, bottom_right: 0.0, bottom_left: 0.0 }, ..Default::default() },
                        ..Default::default()
                    }
                })
                .on_press(Message::SelectGroup(group.id.clone()));
            match drop {
                Some(after) => {
                    let bar = container(Space::new()).width(3).height(28).style(move |_| theme::fill(p.ac, 2.0));
                    let side = if after { ctx.end() } else { ctx.start() };
                    stack([tab.into(), container(bar).width(Length::Fill).height(Length::Fill).align_x(side).align_y(Alignment::End).padding([8, 0]).into()]).into()
                }
                None => tab.into(),
            }
        })
        .collect();
    let add = button(ctx.row(vec![ctx.icon(Icon::Plus, 15.0, p.mu), ctx.text(s.new_group, 13.0, Weight::Normal, p.mu).into()]).spacing(8))
        .padding([8, 12])
        .style(theme::ghost(p.mu, p.bg, 10.0))
        .on_press(Message::OpenModal(ModalKind::Group(None)));
    items.push(container(add).padding(ctx.pad(0.0, 0.0, 6.0, 4.0)).into());
    let strip = ctx.row(items).spacing(4).align_y(Alignment::End).height(58);
    let scroller = scrollable(container(strip).padding([0, 22])).direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(0).scroller_width(0)));
    container(super::smooth::smooth(scroller)).width(Length::Fill).height(58).style(move |_| theme::fill(p.strip, 0.0)).into()
}

/// The picked group: its name, then its cards and the card that adds more.
pub fn view<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let s = ctx.s;
    let data = &ctx.app.snap.data;
    let groups = data.groups();
    let group = ctx.app.shown_group();
    let index = groups.iter().position(|g| g.id == group.id).unwrap_or(0);
    let cards = data.group_cards(&group.id);
    let all: Vec<Card> = cards.iter().map(|(card, _)| (*card).clone()).collect();

    let mut sub = vec![i18n::lines_count(data.group_line_count(&group.id), ctx.lang)];
    if !cards.is_empty() {
        let last = cards.iter().filter_map(|(_, a)| a.updated_at()).max();
        sub.push(i18n::synced(last, ctx.now, ctx.lang));
    }
    let title = column![ctx.kicker(s.home), ctx.bold(group_name(&group, index, s), 34.0).line_height(1.15), ctx.muted(sub.join(" · "), 13.0)]
        .spacing(4)
        .align_x(ctx.start());
    let edit = ctx.tool(Icon::Pencil, s.edit_group, Some(Message::OpenModal(ModalKind::Group(Some(group.id.clone())))), false);
    let head = ctx.row(vec![container(title).width(Length::Fill).align_x(ctx.start()).into(), edit]).align_y(Alignment::Start);

    let mut cells: Vec<El<'a>> = cards.iter().enumerate().map(|(i, (c, account))| card(ctx, c, account, i, &all)).collect();
    cells.push(add_card(ctx, &group.id));
    let width = (ctx.app.main_size.width - 92.0 - 2.0 - 2.0 * 28.0).max(CARD_WIDTH);
    let columns = (((width + GAP) / (CARD_WIDTH + GAP)).floor() as usize).clamp(1, 4);
    let mut rows = column![].spacing(GAP);
    let mut cells = cells.into_iter().peekable();
    while cells.peek().is_some() {
        let row_items: Vec<El<'a>> = (0..columns)
            .map(|_| match cells.next() {
                Some(cell) => container(cell).width(Length::FillPortion(1)).into(),
                None => Space::new().width(Length::FillPortion(1)).into(),
            })
            .collect();
        rows = rows.push(ctx.row(row_items).spacing(GAP).align_y(Alignment::Start));
    }

    let mut page = column![head].spacing(22).padding([28, 28]);
    if cards.is_empty() {
        page = page.push(ctx.muted(s.group_empty, 14.0));
    }
    page = page.push(rows);
    super::smooth::smooth(scrollable(page).style(theme::scroller(ctx.p)).height(Length::Fill))
}

/// A line at a glance: whose and which, what's left of the card's bundle, and when it renews.
fn card<'a>(ctx: &Ctx<'a>, card: &'a Card, account: &'a Account, index: usize, all: &[Card]) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let t = assets::tile(account.carrier);
    let usage = account.usage.as_ref();
    let main = account.main_bundle();
    let shown: Option<&Bundle> = match &card.bundle {
        Some(key) => usage.and_then(|u| u.bundles.iter().find(|b| &b.key == key)),
        None => main,
    };
    let is_main = shown.zip(main).is_some_and(|(b, m)| b.key == m.key);
    // A picked bundle the carrier stopped listing: used up, or not renewed.
    let gone = card.bundle.is_some() && shown.is_none() && usage.is_some();
    let low = shown.is_some_and(|b| account.alert_for(b, is_main).is_triggered(b));
    let dim = shown.is_some_and(|b| matches!(b.state, BundleState::Frozen | BundleState::Inactive));
    let color = if dim { p.mu } else if low { p.danger } else { p.tx };

    let who = column![
        ctx.semi(account.display_name(), 14.5),
        ctx.row(vec![ctx.icon(super::line_icon(account.line), 12.0, p.mu), ctx.num(account.pretty_number(), 12.0, Weight::Normal, p.mu).into()]).spacing(5)
    ]
    .spacing(2)
    .align_x(ctx.start());
    let mut head = vec![tile(account.carrier, 38.0, t.small * 1.3, 11.0, false), container(who).width(Length::Fill).align_x(ctx.start()).into()];
    if ctx.app.snap.is_refreshing(&account.id) {
        head.push(ctx.icon(Icon::Refresh, 14.0, p.mu));
    } else if account.last_error.is_some() {
        head.push(container(Space::new()).width(8).height(8).style(move |_| theme::fill(p.danger, 4.0)).into());
    }

    let amount: El<'a> = match shown {
        Some(m) => {
            let unit = i18n::left(i18n::unit_name(m.unit, ctx.lang), ctx.lang);
            let figures = ctx.row(vec![
                ctx.num(format::amount(m.remaining, m.unit), 28.0, Weight::ExtraBold, color).into(),
                container(ctx.text(unit, 13.0, Weight::Semibold, p.mu)).padding(ctx.pad(0.0, 0.0, 5.0, 0.0)).into(),
                super::fill_x(),
                container(ctx.num(format!("{:.0}%", m.remaining_pct()), 13.0, Weight::Semibold, p.mu)).padding(ctx.pad(0.0, 0.0, 5.0, 0.0)).into(),
            ])
            .spacing(8)
            .align_y(Vertical::Bottom);
            let fill = if dim { p.dv } else if low { p.danger } else { p.ac };
            column![figures, super::bar(m.remaining_pct() / 100.0, 6.0, fill, p.sf, None, ctx.rtl())].spacing(10).into()
        }
        None if gone => column![ctx.semi(kept_name(ctx, card), 14.0), ctx.muted(s.bundle_gone, 13.0)].spacing(2).align_x(ctx.start()).into(),
        None => ctx.muted(if usage.is_some() { "—" } else { s.waiting_first_sync }, 13.0).into(),
    };

    // A bundle's card says which bundle it is, and when it ends if that isn't when the line
    // renews. The main bundle's says the package.
    let renews_at = usage.and_then(|u| u.renews_at);
    let renews = renews_at.map(|at| i18n::renews_in(at, ctx.now, ctx.lang));
    let foot: Vec<String> = match (&card.bundle, shown) {
        (Some(_), Some(b)) => {
            let own_end = b.expires_at.filter(|at| renews_at.is_none_or(|r| (r - *at).num_hours().abs() >= 24));
            let ends = own_end.map(|at| i18n::ends_in(at, ctx.now, ctx.lang));
            std::iter::once(b.name_in(ctx.lang).to_string()).chain(ends.or(renews)).collect()
        }
        (Some(_), None) => Vec::new(),
        (None, _) => usage.and_then(|u| u.package_in(ctx.lang)).map(String::from).into_iter().chain(renews).collect(),
    };
    let content = column![ctx.row(head).spacing(12), amount, ctx.muted(foot.join(" · "), 12.0)].spacing(14).padding(16).width(Length::Fill).height(Length::Fill);

    let me = DragItem::Card(card.clone());
    let (dragged, drop) = drag_marks(ctx, &me, index, |item| match item {
        DragItem::Card(c) => all.iter().position(|x| x.same(c)),
        _ => None,
    });
    // Pressing opens it on release (see `Message::PointerUp`), or drags it to another place.
    let grip = mouse_area(content).on_press(Message::DragStart(me.clone())).on_enter(Message::DragOver(me));
    let card = button(grip)
        .padding(0)
        .width(Length::Fill)
        .height(CARD_HEIGHT)
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(if hovered { p.sf } else { p.bg })),
                border: theme::border(if hovered { p.ac } else if dragged { theme::fade(p.dv, 0.4) } else { p.dv }, 2.0, 16.0),
                text_color: p.tx,
                ..Default::default()
            }
        })
        .on_press(Message::OpenCard(card.clone()));
    match drop {
        // A bar at the side it lands on, over the card so nothing moves under the pointer.
        Some(after) => {
            let bar = container(Space::new()).width(4).height(Length::Fill).style(move |_| theme::fill(p.ac, 2.0));
            let side = if after { ctx.end() } else { ctx.start() };
            stack([card.into(), container(bar).width(Length::Fill).height(Length::Fill).align_x(side).padding([16, 0]).into()]).into()
        }
        None => card.into(),
    }
}

/// The name a bundle's card kept, for when the bundle's gone.
fn kept_name(ctx: &Ctx<'_>, card: &Card) -> String {
    let ar = card.name_ar.as_deref().filter(|_| ctx.rtl());
    ar.or(card.name.as_deref()).unwrap_or_default().to_string()
}

/// The last card: picks the group's lines.
fn add_card<'a>(ctx: &Ctx<'a>, group: &str) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let content = column![
        container(ctx.icon(Icon::Plus, 20.0, p.ac_tx)).center(42).style(move |_| theme::fill(theme::fade(p.ac, 0.22), 21.0)),
        gap(4.0),
        ctx.text(s.add_lines, 14.0, Weight::ExtraBold, p.tx),
        ctx.muted(s.add_lines_hint, 12.0),
    ]
    .spacing(2)
    .align_x(Alignment::Center);
    button(container(content).center(Length::Fill))
        .padding(0)
        .width(Length::Fill)
        .height(CARD_HEIGHT)
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hovered.then_some(Background::Color(p.sf)),
                border: theme::border(if hovered { p.ac } else { p.dvs }, 2.0, 16.0),
                text_color: p.tx,
                ..Default::default()
            }
        })
        .on_press(Message::OpenModal(ModalKind::GroupLines(group.to_string())))
        .into()
}

// ---------------------------------------------------------------------------------------
// Dialogs

pub fn group_view<'a>(ctx: &Ctx<'a>, f: &'a GroupForm) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    // A group left without a name goes back to the one it had at first, so say which.
    let groups = ctx.app.snap.data.groups();
    let unnamed = f.id.as_ref().and_then(|id| groups.iter().position(|g| &g.id == id)).map(|i| group_name(&Group { name: None, ..groups[i].clone() }, i, s));
    let ready = unnamed.is_some() || !f.name.trim().is_empty();
    let mut body = column![column![
        label(ctx, s.group_name),
        gap(6.0),
        field(ctx, unnamed.as_deref().unwrap_or(s.group_ph), &f.name, |t| wrap(Msg::Name(t)), false).on_submit_maybe(ready.then(|| wrap(Msg::Save))).id(GROUP_NAME)
    ]]
    .spacing(16);
    // Home always keeps a tab, so the only one can't go.
    if f.id.is_some() && groups.len() > 1 {
        let delete = ctx
            .row(vec![
                container(ctx.muted(s.delete_group_hint, 12.5)).width(Length::Fill).align_x(ctx.start()).into(),
                button(ctx.text(if f.confirm_delete { s.remove_confirm } else { s.delete_group }, 13.0, Weight::ExtraBold, p.danger))
                    .padding([4, 6])
                    .style(theme::ghost(p.danger, p.sf, 9.0))
                    .on_press(wrap(Msg::Delete))
                    .into(),
            ])
            .spacing(10);
        body = body.push(column![hline(p.dvs, 1.0), gap(12.0), delete]);
    }
    let primary = if f.id.is_some() { s.save } else { s.create };
    let foot = ctx.row(vec![super::fill_x(), ctx.outline(s.cancel, Some(Message::CloseModal)).into(), ctx.primary(primary, ready.then(|| wrap(Msg::Save))).into()]).spacing(8);
    let title = if f.id.is_some() { s.edit_group } else { s.new_group };
    dialog(ctx, 460.0, s.home.to_string(), title.to_string(), Some(Message::CloseModal), body.into(), foot.into())
}

/// Every line Klue checks, by carrier, each with a switch for the group, and its other
/// bundles under it.
pub fn lines_view<'a>(ctx: &Ctx<'a>, f: &'a LinesPicker) -> El<'a> {
    let s = ctx.s;
    let data = &ctx.app.snap.data;
    let groups = data.groups();
    let index = groups.iter().position(|g| g.id == f.group).unwrap_or(0);
    let Some(this) = groups.get(index).filter(|g| g.id == f.group) else { return Space::new().into() };
    let mut body = column![ctx.muted(s.group_lines_hint, 13.0)].spacing(16).align_x(ctx.start());
    let mut any = false;
    for carrier in data.carriers() {
        let rows: Vec<El<'a>> = data.accounts_for(carrier).map(|a| line_rows(ctx, f, this, a)).collect();
        if rows.is_empty() {
            continue;
        }
        any = true;
        body = body.push(column![ctx.caps(i18n::carrier_name(carrier, ctx.lang)), gap(4.0), hline(ctx.p.dv, 2.0), column(rows)].align_x(ctx.start()));
    }
    if !any {
        body = body.push(ctx.muted(s.no_lines_yet, 13.0));
    }
    let foot = ctx.row(vec![super::fill_x(), ctx.primary(s.done, Some(Message::CloseModal)).into()]);
    dialog(ctx, 500.0, group_name(this, index, s), s.group_lines_title.to_string(), Some(Message::CloseModal), body.into(), foot.into())
}

/// A line in the picker: its switch, for its main bundle, then its other bundles when open.
fn line_rows<'a>(ctx: &Ctx<'a>, f: &'a LinesPicker, group: &Group, a: &'a Account) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let switch = |card: Card| ctx.toggle(group.has(&card), wrap(Msg::Toggle(f.group.clone(), card)));
    let t = assets::tile(a.carrier);
    let main = a.main_bundle();
    // Under the number, the bundle its card shows.
    let mut under = vec![ctx.icon(super::line_icon(a.line), 11.0, p.mu), ctx.num(a.pretty_number(), 11.5, Weight::Normal, p.mu).into()];
    if let Some(m) = main {
        under.push(ctx.muted("·", 11.5).into());
        under.push(ctx.icon(Icon::StarFilled, 10.0, p.mu));
        under.push(ctx.muted(m.name_in(ctx.lang).to_string(), 11.5).into());
    }
    let who = column![ctx.semi(a.display_name(), 13.5), ctx.row(under).spacing(5)].spacing(1).align_x(ctx.start());
    let row = ctx.row(vec![tile(a.carrier, 30.0, t.small, 9.0, false), container(who).width(Length::Fill).align_x(ctx.start()).into(), switch(Card::main(&a.id))]).spacing(12);
    let mut block = column![container(row).padding([9, 0])];

    // Every bundle but the main, and any with a card here though it's since become the main
    // or gone.
    let usage = a.usage.as_ref();
    let pinned: Vec<&Card> = group.cards.iter().filter(|c| c.line == a.id && c.bundle.is_some()).collect();
    let listed = |c: &Card| usage.is_some_and(|u| u.bundles.iter().any(|b| c.bundle.as_ref() == Some(&b.key)));
    let main_pinned = main.filter(|m| pinned.iter().any(|c| c.bundle.as_ref() == Some(&m.key)));
    let others: Vec<&Bundle> = main_pinned.into_iter().chain(usage.map(|u| u.ordered(a.main_bundle.as_deref())).unwrap_or_default().into_iter().skip(1)).collect();
    let gone: Vec<&Card> = pinned.into_iter().filter(|c| !listed(c)).collect();
    let count = others.len() + gone.len();
    if count > 0 {
        let open = f.open.contains(&a.id);
        let label = ctx
            .row(vec![ctx.text(format!("{} ({count})", s.other_bundles), 12.5, Weight::Semibold, p.ac_tx).into(), ctx.icon(if open { Icon::ChevronUp } else { Icon::ChevronDown }, 13.0, p.ac_tx)])
            .spacing(4)
            .align_y(Alignment::Center);
        let expand = button(label).padding([3, 6]).style(theme::ghost(p.ac_tx, p.sf, 8.0)).on_press(wrap(Msg::Expand(a.id.clone())));
        // Lined up under the name, past the tile.
        block = block.push(container(expand).padding(ctx.pad(0.0, 0.0, 8.0, 36.0)));
        if open {
            let bundle_row = |name: String, detail: String, card: Card| -> El<'a> {
                let text = column![ctx.semi(name, 13.0), ctx.muted(detail, 12.0)].spacing(0).align_x(ctx.start());
                container(ctx.row(vec![container(text).width(Length::Fill).align_x(ctx.start()).into(), switch(card)]).spacing(12)).padding(ctx.pad(6.0, 0.0, 6.0, 42.0)).into()
            };
            for b in others {
                let left = i18n::left(&format!("{} {}", format::amount(b.remaining, b.unit), i18n::unit_name(b.unit, ctx.lang)), ctx.lang);
                block = block.push(bundle_row(b.name_in(ctx.lang).to_string(), left, Card::bundle(&a.id, b)));
            }
            for c in gone {
                block = block.push(bundle_row(kept_name(ctx, c), s.bundle_gone.to_string(), c.clone()));
            }
            block = block.push(gap(8.0));
        }
    }
    block.push(hline(p.dvs, 1.0)).into()
}
