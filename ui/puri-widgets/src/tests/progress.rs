use super::*;

#[test]
fn fill_uses_the_supplied_geometry_and_bounded_fraction() {
    let rect = Rect::new(10.0, 20.0, 210.0, 24.0);
    let mut bar = ProgressBar {
        fraction: 0.25,
        track: Color::BLACK,
        fill: Color::WHITE,
    };
    assert_eq!(bar.fill_rect(rect), Rect::new(10.0, 20.0, 60.0, 24.0));
    bar.fraction = 2.0;
    assert_eq!(bar.fill_rect(rect), rect);
    bar.fraction = f64::NAN;
    assert_eq!(bar.fill_rect(rect).width(), 0.0);
}
