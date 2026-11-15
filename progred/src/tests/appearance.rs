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

#[cfg(target_os = "linux")]
#[test]
fn the_portals_color_scheme_prefers_dark_light_or_neither() {
    assert_eq!(color_scheme(0), None);
    assert_eq!(color_scheme(1), Some(Theme::Dark));
    assert_eq!(color_scheme(2), Some(Theme::Light));
    assert_eq!(color_scheme(3), None);
}
