use super::*;
use crate::libraries::{color, name};
use std::collections::BTreeSet;

#[test]
fn the_css_palette_has_unique_durable_cells_and_expected_aliases() {
    assert_eq!(NAMED.len(), 148);
    assert_eq!(
        NAMED
            .iter()
            .map(|color| color.id)
            .chain([TRANSPARENT])
            .collect::<BTreeSet<_>>()
            .len(),
        149,
    );

    let named = |wanted| {
        colors()
            .find(|(_, value)| name::read(value) == Some(wanted))
            .and_then(|(_, value)| color::read(&value))
    };
    assert_eq!(
        named("rebeccapurple"),
        Some(peniko::Color::from_rgb8(0x66, 0x33, 0x99))
    );
    assert_eq!(named("aqua"), named("cyan"));
    assert_eq!(named("transparent"), Some(peniko::Color::TRANSPARENT));
}
