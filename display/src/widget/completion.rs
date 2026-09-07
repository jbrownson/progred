//! Completion interaction is a native widget; callers supply offers and own state.

use super::{Fragment, container, extent, leaf, scroll, style::Styles};
use measured::{Extent, Measured, col, pad};
use peniko::kurbo::Insets;
use puri::handler::{HasHandler, Key, Modifiers, NamedKey, PointerType};
use puri::hover::Probe;
use puri::text::{TextCtx, TextStyle};
use puri::{Color, Point, Size, Stroke, Vec2};
use puri_widgets::panel::Panel;
use std::ops::Range;
use std::rc::Rc;

pub struct Entry<'a, World, Hover> {
    pub display: &'a str,
    pub detail: Option<&'a str>,
    pub matches: &'a [Range<usize>],
    pub style: &'a TextStyle,
    pub target: Hover,
    pub activate: Rc<dyn Fn(&mut World)>,
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct State {
    pub choice: usize,
    pub scroll: f64,
    pub everything: bool,
}

pub fn border(scale: f64) -> Stroke {
    Stroke::new(scale)
}

pub fn card<C: 'static, H: Clone + PartialEq + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
    entries: &[Entry<'_, C, H>],
    more: H,
    state: State,
    set_view: impl Fn(&mut C, State) + 'static,
    command: fn(&Modifiers) -> bool,
) -> Measured<Fragment<C, H>> {
    let State {
        choice,
        scroll,
        everything,
    } = state;
    let set_view = move |world: &mut C, scroll, choice, everything| {
        set_view(
            world,
            State {
                scroll,
                choice,
                everything,
            },
        )
    };
    let scale = styles.scale;
    let widget_entries = entries.iter().map(|entry| puri_widgets::completion::Entry {
        display: entry.display,
        detail: entry.detail,
        matches: entry.matches,
        style: entry.style,
    });
    let widget = puri_widgets::completion::Completion::new(
        tcx,
        widget_entries,
        !everything,
        puri_widgets::completion::Style {
            detail: &styles.detail,
            more: &styles.dim,
            scale,
            chosen: Color::new([0.0, 0.48, 1.0, 0.14]),
        },
    );
    let set_view = Rc::new(set_view);
    let expand: Rc<dyn Fn(&mut C)> = {
        let set_view = set_view.clone();
        Rc::new(move |world| {
            set_view(world, scroll, choice, true);
        })
    };
    let items = widget
        .rows
        .into_iter()
        .zip(entries)
        .map(|(row, entry)| (row, entry.target.clone(), entry.activate.clone()))
        .chain(widget.more.map(|row| (row, more, expand.clone())))
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
                move |world| set_view(world, scroll, index, everything),
                move |world| activate(world),
            )
        })
        .collect::<Vec<_>>();
    let gap = 2.0 * scale;
    let row_spans = row_spans(&rows, gap, scale);
    let content = col(0, gap, rows);
    let viewport_height = viewport_height(&row_spans);
    let maximum = (content.extent.height() / scale - viewport_height).max(0.0);
    let scroll = scroll.clamp(0.0, maximum);
    let viewport_extent = Extent {
        width: content.extent.width,
        ascent: content.extent.ascent.min(viewport_height * scale),
        descent: (viewport_height * scale - content.extent.ascent).max(0.0),
    };
    let scroll_view = set_view.clone();
    let scrolled = container::scrolled(
        content,
        Vec2::new(0.0, scroll * scale),
        move |world, event| {
            let (next, outcome) = scroll::offset(
                Vec2::new(0.0, scroll),
                event,
                scale,
                Size::new(viewport_extent.width, viewport_extent.height()),
                Vec2::new(0.0, maximum),
            );
            if next.y != scroll {
                scroll_view(world, next.y, choice, everything);
            }
            outcome
        },
    );
    let viewport = measured::overlay(
        leaf(viewport_extent, |_, _| {}),
        scrolled,
        move |placement, _, _| Some(placement),
    );
    let card = pad(Insets::uniform(4.0 * scale), viewport);
    let card = measured::before_into(card, move |_, output: &mut Fragment<C, H>| {
        output.handler().on_key(move |world, event| {
            if event.state.is_down() {
                match event.key {
                    Key::Named(NamedKey::Enter) => activate.as_ref().is_some_and(|activate| {
                        activate(world);
                        true
                    }),
                    Key::Named(key @ (NamedKey::Tab | NamedKey::ArrowDown))
                        if !everything
                            && !command(&event.modifiers)
                            && (key == NamedKey::Tab || choice == count.saturating_sub(1)) =>
                    {
                        expand(world);
                        true
                    }
                    Key::Named(direction @ (NamedKey::ArrowUp | NamedKey::ArrowDown))
                        if !command(&event.modifiers) =>
                    {
                        let next = match direction {
                            NamedKey::ArrowUp => choice.saturating_sub(1),
                            _ => choice.saturating_add(1).min(count.saturating_sub(1)),
                        };
                        set_view(
                            world,
                            reveal(scroll, next, &row_spans, viewport_height).clamp(0.0, maximum),
                            next,
                            everything,
                        );
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        });
    });
    let panel = Panel {
        fill: Some(Color::WHITE.into()),
        border: Some((border(scale), Color::new([0.75, 0.77, 0.81, 1.0]).into())),
        radius: 6.0 * scale,
    };
    measured::before_into(card, move |placement, output| {
        if !placement.clipped_out() {
            output.render(move |canvas, _| panel.place(canvas, placement));
            output.claims.push(Probe::occludes(placement));
            output.handler().on_pointer_down(move |_, event| {
                placement.contains(Point::new(event.state.position.x, event.state.position.y))
            });
        }
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
) -> Measured<Fragment<C, H>> {
    leaf(extent(row.metrics()), move |output, placement| {
        if !placement.clipped_out() {
            output
                .claims
                .push(Probe::retaining(placement, hover.clone()));
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
                        && hovered.as_ref() == Some(&hover)
                        && {
                            activate(world);
                            true
                        }
                });
        }
        output.render(move |canvas, _| row.draw(canvas, placement, chosen));
    })
}
