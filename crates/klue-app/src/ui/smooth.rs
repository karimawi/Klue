//! Smooth wheel scrolling. iced jumps 60 px per wheel notch (what Windows mice send),
//! which looks jittery; this spreads each notch over a few frames with an ease-out.
//! Touchpads send pixel deltas and pass straight through.
//!
//! While scrolling it also adds an empty layer that makes tiny-skia redraw the window in
//! one pass instead of one pass per moved rectangle, which was much slower.

use std::time::{Duration, Instant};

use iced::{
    Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget, layout, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    keyboard, mouse, window,
};

use super::El;
use crate::app::Message;

/// How far iced scrolls for one line.
const LINE: f32 = 60.0;
/// Ease-out time constant in seconds: a notch is 95% done after 3τ (~165 ms).
const TAU: f32 = 0.055;
/// About 60 fps.
const FRAME: Duration = Duration::from_millis(16);

/// `content` (a scrollable) with eased wheel scrolling.
pub fn smooth<'a>(content: impl Into<El<'a>>) -> El<'a> {
    Element::new(Smooth { content: content.into() })
}

struct Smooth<'a> {
    content: El<'a>,
}

#[derive(Default)]
struct State {
    /// Distance left to scroll, in pixels.
    pending: Vector,
    /// Where the wheel turned; steps go to whatever scrolls under it.
    anchor: Option<Point>,
    last_frame: Option<Instant>,
    modifiers: keyboard::Modifiers,
    /// A touchpad moved the content since the last frame.
    nudged: bool,
    /// Whether this frame moves the content, and a count of such frames (see [`redraw_all`]).
    moving: bool,
    frame: u8,
}

/// How much of `pending` to scroll in a frame of `dt`; all of it once under half a pixel.
fn step(pending: Vector, dt: Duration) -> Vector {
    let k = 1.0 - (-dt.as_secs_f32() / TAU).exp();
    let step = pending * k;
    let rest = pending - step;
    if rest.x.abs() < 0.5 && rest.y.abs() < 0.5 { pending } else { step }
}

/// Bounds of an empty layer that makes tiny-skia redraw the whole window in one pass.
///
/// A layer whose bounds changed counts as fully damaged. The renderer sorts damaged areas
/// by distance from the top left and merges each into the previous one, so a huge area
/// centred there swallows the rest. The width cycles through 8 values so it also differs
/// from the last few frames.
fn redraw_all(frame: u8) -> Rectangle {
    const HALF: f32 = 100_000.0;
    Rectangle { x: -HALF, y: -HALF, width: 2.0 * HALF + f32::from(frame % 8), height: 2.0 * HALF }
}

/// iced's line-to-pixel mapping: Shift scrolls sideways (macOS swaps the axes itself).
fn lines_to_pixels(x: f32, y: f32, shift: bool) -> Vector {
    let (x, y) = if cfg!(target_os = "macos") && shift { (y, x) } else { (x, y) };
    let movement = if shift { Vector::new(y, x) } else { Vector::new(x, y) };
    movement * LINE
}

impl Widget<Message, iced::Theme, iced::Renderer> for Smooth<'_> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        if state.moving {
            renderer.with_layer(redraw_all(state.frame), |_| {});
        }
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn Operation) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            state.moving = state.pending != Vector::ZERO || std::mem::take(&mut state.nudged);
            if state.moving {
                state.frame = state.frame.wrapping_add(1);
            }
        }
        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => state.modifiers = *modifiers,
            Event::Mouse(mouse::Event::WheelScrolled { delta: mouse::ScrollDelta::Pixels { .. } })
                if !shell.is_event_captured() && cursor.is_over(layout.bounds()) =>
            {
                state.nudged = true;
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta: mouse::ScrollDelta::Lines { x, y } })
                if !shell.is_event_captured() && cursor.is_over(layout.bounds()) =>
            {
                state.pending += lines_to_pixels(*x, *y, state.modifiers.shift());
                state.anchor = cursor.position();
                shell.capture_event();
                shell.request_redraw();
                return;
            }
            Event::Window(window::Event::RedrawRequested(now)) if state.pending != Vector::ZERO => {
                let dt = state.last_frame.map_or(Duration::from_millis(16), |t| now.saturating_duration_since(t)).min(Duration::from_millis(100));
                let delta = step(state.pending, dt);
                state.pending -= delta;
                if state.pending == Vector::ZERO {
                    state.last_frame = None;
                } else {
                    state.last_frame = Some(*now);
                    shell.request_redraw_at(*now + FRAME);
                }
                let wheel = Event::Mouse(mouse::Event::WheelScrolled { delta: mouse::ScrollDelta::Pixels { x: delta.x, y: delta.y } });
                let at = state.anchor.map_or(cursor, mouse::Cursor::Available);
                // Own shell: the scrollable captures the step, but the redraw event still has to reach
                // the rest of the window.
                let mut messages = Vec::new();
                let mut local = Shell::new(&mut messages);
                self.content.as_widget_mut().update(&mut tree.children[0], &wheel, layout, at, renderer, clipboard, &mut local, viewport);
                let (relayout, rebuild, redraw) = (local.is_layout_invalid(), local.are_widgets_invalid(), local.redraw_request());
                drop(local);
                messages.into_iter().for_each(|m| shell.publish(m));
                if relayout {
                    shell.invalidate_layout();
                }
                if rebuild {
                    shell.invalidate_widgets();
                }
                shell.request_redraw_at(redraw);
            }
            _ => {}
        }
        self.content.as_widget_mut().update(&mut tree.children[0], event, layout, cursor, renderer, clipboard, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notch_eases_out_over_a_few_frames_and_lands_exactly() {
        let mut pending = lines_to_pixels(0.0, -1.0, false);
        assert_eq!(pending, Vector::new(0.0, -LINE));
        let mut steps = Vec::new();
        while pending != Vector::ZERO {
            let d = step(pending, Duration::from_millis(16));
            steps.push(d.y);
            pending -= d;
        }
        // Biggest step first, done well under half a second at 60 fps, adding up to one notch.
        assert!(steps[0] < -10.0 && steps[0] > -LINE);
        assert!(steps.iter().all(|s| s.abs() <= steps[0].abs()));
        assert!(steps.len() < 20, "took {} frames", steps.len());
        assert!((steps.iter().sum::<f32>() + LINE).abs() < 1e-3);
        // A slow frame covers more ground, but never overshoots.
        assert_eq!(step(Vector::new(0.0, 6.0), Duration::from_secs(1)), Vector::new(0.0, 6.0));
    }

    #[test]
    fn the_redraw_layer_comes_first_and_changes_every_frame() {
        let bounds: Vec<Rectangle> = (0..8).map(redraw_all).collect();
        // Centred on the window's top left, so the renderer takes it before anything else.
        assert!(bounds.iter().all(|b| b.center().distance(Point::ORIGIN) < 4.0));
        // Different from each of the 7 frames before it.
        for (i, a) in bounds.iter().enumerate() {
            assert!(bounds[i + 1..].iter().all(|b| a != b));
        }
        assert_eq!(redraw_all(8), redraw_all(0));
    }

    #[test]
    fn shift_scrolls_sideways() {
        let v = lines_to_pixels(0.0, 1.0, true);
        if cfg!(target_os = "macos") {
            assert_eq!(v, Vector::new(0.0, LINE));
        } else {
            assert_eq!(v, Vector::new(LINE, 0.0));
        }
    }
}
