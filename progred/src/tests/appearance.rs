use super::*;

#[test]
fn a_chosen_theme_overrides_the_systems_and_light_is_the_fallback() {
    use Theme::{Dark, Light};
    for (chosen, system, expected) in [
        (None, Some(Dark), Dark),
        (None, Some(Light), Light),
        (None, None, Light),
        (Some(Dark), Some(Light), Dark),
        (Some(Light), Some(Dark), Light),
        (Some(Dark), None, Dark),
    ] {
        assert_eq!(resolve(chosen, system), expected);
    }
    assert_eq!(palette(Dark), style::Theme::Dark.palette());
    assert_eq!(palette(Light), style::Theme::Light.palette());
}
