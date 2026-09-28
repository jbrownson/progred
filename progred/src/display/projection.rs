//! Selectable whole-value boundaries and common head/body presentations.
//! Navigation order comes from the ordinary row/column layout.

use super::{Layout, alternatives, col, pad, row, shared};

pub use super::widget::navigation::{nav_group as group, target};

/// Prefer head beside body; otherwise put an indented body below the head.
/// Navigation follows only the chosen presentation, never both alternatives.
pub fn hug<C: 'static, H: Clone + 'static>(
    head: Layout<C, H>,
    child: Layout<C, H>,
    gap: f64,
    tab: f64,
) -> Layout<C, H> {
    let head = shared(head);
    let child = shared(child);
    alternatives([
        row(gap, [head.clone(), child.clone()]),
        col(0, 2.0, [head, pad(tab, child)]),
    ])
}

/// A selectable whole around either head/body presentation.
pub fn group_hug(
    head: Layout<crate::Editor, crate::frame::Hovered>,
    child: Layout<crate::Editor, crate::frame::Hovered>,
    gap: f64,
    tab: f64,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    group(hug(head, child, gap, tab))
}
