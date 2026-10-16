use super::*;

#[test]
fn placement_uses_the_ancestor_clip_and_skips_an_offscreen_anchor() {
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let extent = Extent {
        width: 30.0,
        ascent: 0.0,
        descent: 40.0,
    };
    let place = |rect| position(Placement::new(rect, bounds), extent, 4.0);
    assert_eq!(
        place(Rect::new(85.0, 5.0, 95.0, 15.0)),
        Some(Placement::new(Rect::new(70.0, 19.0, 100.0, 59.0), bounds))
    );
    assert_eq!(
        place(Rect::new(85.0, 85.0, 95.0, 95.0)),
        Some(Placement::new(Rect::new(70.0, 41.0, 100.0, 81.0), bounds))
    );
    assert_eq!(place(Rect::new(110.0, 0.0, 120.0, 10.0)), None);
}

#[test]
fn a_card_that_fits_neither_side_shortens_on_the_roomier_one() {
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let size = Size::new(30.0, 60.0);
    let place = |y0, y1| shortened(Rect::new(10.0, y0, 40.0, y1), size, bounds, 0.0, 10.0);
    // Whole below when it fits there, else whole above.
    assert_eq!(place(10.0, 20.0), Rect::new(10.0, 20.0, 40.0, 80.0));
    assert_eq!(place(70.0, 80.0), Rect::new(10.0, 10.0, 40.0, 70.0));
    // Neither: as much as fits on the roomier side, clear of the anchor.
    assert_eq!(place(40.0, 50.0), Rect::new(10.0, 50.0, 40.0, 100.0));
    assert_eq!(place(55.0, 65.0), Rect::new(10.0, 0.0, 40.0, 55.0));
    // Never shorter than the minimum, even if it then covers the anchor.
    let cramped = Rect::new(0.0, 0.0, 100.0, 30.0);
    assert_eq!(
        shortened(Rect::new(10.0, 10.0, 40.0, 20.0), size, cramped, 0.0, 15.0),
        Rect::new(10.0, 15.0, 40.0, 30.0)
    );
}
