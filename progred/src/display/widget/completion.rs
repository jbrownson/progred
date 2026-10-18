//! Completion interaction is a native widget; callers supply offers and own state.

use crate::display::widget::{HoverContext, HoverPass};
use crate::hover::Hover;

use super::frame::Probe;
use super::offers::Entry;
use super::style::{Styles, face_style, primary_highlight_stroke};
use super::{container, extent, leaf, popover, scroll};
use kurbo::{Insets, Point, Size, Stroke, Vec2};
use measured::{Extent, Measured, col, pad};
use puri::handler::HasHandler;
use puri::text::TextCtx;
use puri_widgets::panel::Panel;
use puri_widgets::text_frame;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};
use ui_events::pointer::PointerType;
use uig::Placement;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct State {
    pub choice: usize,
    pub scroll: f64,
}

fn border(scale: f64) -> Stroke {
    Stroke::new(scale)
}

/// Where the card for a query drawn in `field` floats: against the field's
/// selection ring, below it or above, as [`popover::shortened`] decides.
/// None when the field is clipped out.
pub fn placement(field: Placement, extent: Extent, scale: f64) -> Option<Placement> {
    (!field.clipped_out()).then(|| {
        let ring_outset = primary_highlight_stroke(scale).width / 2.0;
        let anchor = text_frame::outline(scale, field.rect)
            .rect()
            .inflate(ring_outset, ring_outset);
        let border_outset = border(scale).width / 2.0;
        // Shortened, the card scrolls what it can't show; it keeps at least
        // about a row, the field's own height.
        let outer = popover::shortened(
            anchor,
            Size::new(
                extent.width + 2.0 * border_outset,
                extent.height() + 2.0 * border_outset,
            ),
            field.clip_rect,
            0.0,
            anchor.height(),
        );
        Placement::new(
            outer.inflate(-border_outset, -border_outset),
            field.clip_rect,
        )
    })
}

/// A card listing `entries`, `state.choice` chosen. Each row claims its index
/// among them as [`Hover::Entry`], so the caller retains the same entries as
/// the frame's offers for the index to name.
pub fn card<C: 'static, H: Clone + PartialEq + From<Hover> + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
    entries: &[Entry<C>],
    state: State,
    current_scroll: impl Fn(&mut C) -> Option<f64> + 'static,
    set_view: impl Fn(&mut C, State) + 'static,
) -> Measured<HoverPass<C, H>> {
    let State { choice, scroll } = state;
    let set_view = move |world: &mut C, scroll, choice| set_view(world, State { scroll, choice });
    let scale = styles.scale;
    let widget_entries = entries.iter().map(|entry| puri_widgets::completion::Entry {
        display: &entry.display,
        detail: entry.detail.as_deref(),
        matches: &entry.matches,
        style: face_style(styles, entry.face),
    });
    let widget_rows = puri_widgets::completion::rows(
        tcx,
        widget_entries,
        puri_widgets::completion::Style {
            detail: &styles.detail,
            scale,
            chosen: styles.palette.accent.with_alpha(0.14),
        },
    );
    let set_view = Rc::new(set_view);
    let items = widget_rows
        .into_iter()
        .zip(entries)
        .enumerate()
        .map(|(index, (row, entry))| (row, H::from(Hover::Entry(index)), entry.activate.clone()))
        .collect::<Vec<_>>();
    let count = items.len();
    let choice = choice.min(count.saturating_sub(1));
    let activate = items.get(choice).map(|(_, _, activate)| activate.clone());
    let rows = items
        .into_iter()
        .enumerate()
        .map(|(index, (row, hover, activate))| {
            let set_view = set_view.clone();
            completion_row(
                row,
                hover,
                index == choice,
                move |world| set_view(world, scroll, index),
                move |world| activate(world),
            )
        })
        .collect::<Vec<_>>();
    let gap = 2.0 * scale;
    let row_spans = row_spans(&rows, gap, scale);
    let content = col(0, gap, rows);
    let inset = 4.0 * scale;
    // Measured at most eight rows tall. Placed shorter, the card shows the
    // rows that fit and scrolls the rest.
    let tallest = viewport_height(&row_spans);
    let tallest_extent = viewport_extent(content.extent, tallest, scale);
    let extent = Extent {
        width: tallest_extent.width + 2.0 * inset,
        ascent: tallest_extent.ascent + inset,
        descent: tallest_extent.descent + inset,
    };
    let panel = Panel {
        fill: Some(styles.palette.panel.into()),
        border: Some((border(scale), styles.palette.border.into())),
        radius: 6.0 * scale,
    };
    measured::leaf_into(extent, move |placement, pass| {
        let viewport_height = ((placement.rect.height() - 2.0 * inset) / scale).clamp(0.0, tallest);
        let maximum = (content.extent.height() / scale - viewport_height).max(0.0);
        let scroll = scroll.clamp(0.0, maximum);
        let visible = viewport_extent(content.extent, viewport_height, scale);
        let scroll_view = set_view.clone();
        let scrolled = container::scrolled(
            content,
            Vec2::new(0.0, scroll * scale),
            scale,
            move |world, event| {
                if let Some(scroll) = current_scroll(world) {
                    let (next, outcome) = scroll::offset(
                        Vec2::new(0.0, scroll),
                        event,
                        scale,
                        Size::new(visible.width, visible.height()),
                        Vec2::new(0.0, maximum),
                    );
                    if next.y != scroll {
                        scroll_view(world, next.y, choice);
                    }
                    outcome
                } else {
                    puri::handler::ScrollOutcome::pass(event)
                }
            },
        );
        let viewport =
            measured::overlay_into(leaf(visible, |_, _| {}), scrolled, move |placement, _| {
                Some(placement)
            });
        let card = pad(Insets::uniform(inset), viewport);
        let card = crate::display::widget::before_place(
            card,
            move |_, output: &mut HoverContext<'_, C, H>| {
                let command = output.input.command_modifier;
                output.handler().on_key(move |world, event| {
                    if event.state.is_down() {
                        match event.key {
                            Key::Named(NamedKey::Enter) => {
                                activate.as_ref().is_some_and(|activate| {
                                    activate(world);
                                    true
                                })
                            }
                            Key::Named(direction @ (NamedKey::ArrowUp | NamedKey::ArrowDown))
                                if !command.pressed(&event.modifiers) =>
                            {
                                let next = match direction {
                                    NamedKey::ArrowUp => choice.saturating_sub(1),
                                    _ => choice.saturating_add(1).min(count.saturating_sub(1)),
                                };
                                set_view(
                                    world,
                                    reveal(scroll, next, &row_spans, viewport_height)
                                        .clamp(0.0, maximum),
                                    next,
                                );
                                true
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                });
            },
        );
        let card = crate::display::widget::before_place(card, move |placement, output| {
            if !placement.clipped_out() {
                output.render(move |canvas, _| panel.place(canvas, placement));
                output.claim(Probe::occludes(placement));
                output.handler().on_pointer_down(move |_, event| {
                    placement.contains(Point::new(event.state.position.x, event.state.position.y))
                });
            }
        });
        measured::place_into(card, placement, pass);
    })
}

fn row_spans<O>(rows: &[Measured<O>], gap: f64, scale: f64) -> Vec<(f64, f64)> {
    rows.iter()
        .scan(0.0, |top, row| {
            let span = (*top, *top + row.extent.height() / scale);
            *top = span.1 + gap / scale;
            Some(span)
        })
        .collect()
}

fn viewport_height(spans: &[(f64, f64)]) -> f64 {
    const VISIBLE_ROWS: usize = 8;
    spans
        .get(
            VISIBLE_ROWS
                .saturating_sub(1)
                .min(spans.len().saturating_sub(1)),
        )
        .map_or(0.0, |(_, bottom)| *bottom)
}

/// A viewport `height` tall onto `content`, keeping its first baseline when
/// there's room for it.
fn viewport_extent(content: Extent, height: f64, scale: f64) -> Extent {
    Extent {
        width: content.width,
        ascent: content.ascent.min(height * scale),
        descent: (height * scale - content.ascent).max(0.0),
    }
}

fn reveal(scroll: f64, choice: usize, spans: &[(f64, f64)], viewport_height: f64) -> f64 {
    match spans.get(choice) {
        Some((top, _)) if *top < scroll => *top,
        Some((_, bottom)) if *bottom > scroll + viewport_height => *bottom - viewport_height,
        _ => scroll,
    }
}

fn completion_row<C: 'static, H: Clone + PartialEq + 'static>(
    row: puri_widgets::completion::Row,
    hover: H,
    chosen: bool,
    choose: impl Fn(&mut C) + 'static,
    activate: impl Fn(&mut C) + 'static,
) -> Measured<HoverPass<C, H>> {
    leaf(extent(row.metrics()), move |output, placement| {
        if !placement.clipped_out() {
            output.claim(Probe::retaining(placement, hover.clone()));
            output.handler().on_pointer_move(move |world, event| {
                if event.pointer.pointer_type == PointerType::Mouse
                    && event.current.buttons.is_empty()
                    && placement.contains(Point::new(
                        event.current.position.x,
                        event.current.position.y,
                    ))
                {
                    choose(world);
                    true
                } else {
                    false
                }
            });
            output
                .handler()
                .on_pointer_down_with(move |world, event, hovered| {
                    puri::interact::is_primary_contact(event)
                        && hovered.hovered() == Some(&hover)
                        && {
                            activate(world);
                            true
                        }
                });
        }
        output.render(move |canvas, _| row.draw(canvas, placement, chosen));
    })
}

#[cfg(test)]
mod tests;
