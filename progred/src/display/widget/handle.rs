//! A handle: a mark an occurrence draws of its own where what's inside it
//! would otherwise draw all of it, so selecting the occurrence looks
//! different from selecting what's inside, and a click on the mark selects
//! the occurrence.

use super::{Widget, fill_height, leaf, selectable_widget};
use crate::display::Host;
use crate::display::host::Layout;
use measured::Extent;
use std::rc::Rc;

/// `child` after a bar as tall as it, in the border's color, or the accent's
/// while the bar is hovered or the occurrence selected.
pub fn handle<W: Host>(child: Layout<W>) -> Layout<W> {
    crate::display::row(0.0, [bar(Bar::Always), child])
}

/// A thinner [`handle`], shown only while it's hovered or the selection is
/// on the occurrence or inside it, for an occurrence as common as a
/// parameter's name, where a bar on each would crowd what's drawn. Its room
/// is kept on both sides of `child`, so nothing moves when it shows and
/// `child` sits evenly between its neighbours.
pub fn quiet_handle<W: Host>(child: Layout<W>) -> Layout<W> {
    crate::display::row(0.0, [bar(Bar::Quiet), child, room(Bar::Quiet)])
}

#[derive(Clone, Copy, PartialEq)]
enum Bar {
    Always,
    Quiet,
}

impl Bar {
    /// The bar's width and the room it takes, in logical points.
    fn size(self) -> (f64, f64) {
        match self {
            Bar::Always => (2.0, 6.0),
            Bar::Quiet => (1.5, 3.0),
        }
    }
}

fn bar<W: Host>(kind: Bar) -> Layout<W> {
    let widget: Widget<W, W::Hover> = Rc::new(move |context| {
        let styles = context.toolkit.styles();
        let scale = styles.scale;
        let (width, room) = kind.size();
        let (width, room, inset) = (width * scale, room * scale, scale);
        let (rest, hot) = (styles.palette.border, styles.palette.accent);
        let selected = context.toolkit.selected();
        let shown = kind == Bar::Always || context.toolkit.selected_below(&[]);
        let target = W::Hover::from(context.site().hover());
        leaf(
            Extent {
                width: room,
                ascent: 0.0,
                descent: 0.0,
            },
            move |output, placement| {
                output.render(move |canvas, hovered| {
                    let hovered = hovered.hovered.as_ref() == Some(&target);
                    if !shown && !hovered {
                        return;
                    }
                    let rect = placement.rect;
                    let bar = kurbo::RoundedRect::new(
                        rect.x0,
                        rect.y0 + inset,
                        rect.x0 + width,
                        rect.y1 - inset,
                        width / 2.0,
                    );
                    let color = if hovered || selected { hot } else { rest };
                    canvas.fill_shape(bar.into(), color.into(), kurbo::Affine::IDENTITY);
                });
            },
        )
    });
    Layout::widget(fill_height(selectable_widget(widget)))
}

/// Blank room as wide as `kind`'s bar takes.
fn room<W: Host>(kind: Bar) -> Layout<W> {
    Layout::widget(Rc::new(move |context| {
        let width = kind.size().1 * context.toolkit.styles().scale;
        leaf(
            Extent {
                width,
                ascent: 0.0,
                descent: 0.0,
            },
            |_, _| {},
        )
    }))
}
