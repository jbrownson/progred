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
    marked(child, false)
}

/// [`handle`], shown only while its bar is hovered or the selection is on
/// the occurrence or inside it, for an occurrence as common as a
/// parameter's name, where a bar on each would crowd what's drawn. Its
/// place is kept, so nothing moves when it shows.
pub fn quiet_handle<W: Host>(child: Layout<W>) -> Layout<W> {
    marked(child, true)
}

fn marked<W: Host>(child: Layout<W>, quiet: bool) -> Layout<W> {
    crate::display::row(
        0.0,
        [
            Layout::widget(fill_height(selectable_widget(bar(quiet)))),
            child,
        ],
    )
}

fn bar<W: Host>(quiet: bool) -> Widget<W, W::Hover> {
    Rc::new(move |context| {
        let styles = context.toolkit.styles();
        let scale = styles.scale;
        let (width, gap, inset) = (2.0 * scale, 4.0 * scale, scale);
        let (rest, hot) = (styles.palette.border, styles.palette.accent);
        let selected = context.toolkit.selected();
        let shown = !quiet || context.toolkit.selected_below(&[]);
        let target = W::Hover::from(context.site().hover());
        leaf(
            Extent {
                width: width + gap,
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
    })
}
