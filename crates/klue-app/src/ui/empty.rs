//! Empty state: no numbers yet.

use iced::{
    Alignment, Length,
    font::Weight,
    widget::{button, column, container},
};
use klue_core::store::Lang;

use super::{Ctx, El, hline};
use crate::{
    app::{Message, ModalKind},
    assets::Icon,
    theme,
};

pub fn view<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let (title, cta) = match ctx.lang {
        Lang::En => ("No numbers yet", "Add a number"),
        Lang::Ar => ("لا توجد أرقام بعد", "إضافة رقم"),
    };
    let steps: Vec<El<'a>> = s
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            container(column![ctx.num(format!("0{}", i + 1), 22.0, Weight::ExtraBold, p.ac_tx), ctx.body(*step, 13.0)].spacing(6).align_x(ctx.start()))
                .padding(ctx.pad(14.0, 14.0, 16.0, 0.0))
                .width(Length::FillPortion(1))
                .into()
        })
        .collect();
    let cta = button(ctx.row(vec![ctx.icon(Icon::Plus, 16.0, p.on), ctx.text(cta, 14.0, Weight::ExtraBold, p.on).into()]).spacing(10))
        .padding([11, 16])
        .style(theme::primary(p))
        .on_press(Message::OpenModal(ModalKind::Add(None)));
    let inner = column![
        ctx.kicker(s.empty_kicker),
        ctx.bold(title, 44.0).line_height(1.1),
        ctx.muted(s.empty_body, 15.0).line_height(1.6),
        column![hline(p.dv, 2.0), ctx.row(steps).align_y(Alignment::Start), hline(p.dv, 2.0)],
        cta,
    ]
    .spacing(18)
    .max_width(560)
    .align_x(ctx.start());
    container(inner).padding([48, 56]).width(Length::Fill).height(Length::Fill).align_y(Alignment::Center).align_x(ctx.start()).into()
}
