use super::*;
use crate::display::test_support::painted;
use crate::display::widget::style::{self, Palette};
use crate::driver::Driver;
use crate::libraries::text;
use puri::draw::DrawCmd;
use ui_events::keyboard::Key;

/// A list of text and a list in a window at `zoom`, the inner list selected;
/// its path.
fn selecting_a_list(zoom: f64) -> (Driver, Path) {
    let mut editor = crate::editor_of(Value::list([
        text::value("first"),
        Value::list([text::value("inner"), text::value("list")]),
    ]));
    editor.zoom = zoom;
    let mut driver = Driver::new(editor, Driver::VIEWPORT);
    let path = driver.element(&[], 1, &[]);
    driver.select(&path);
    (driver, path)
}

/// Press the command key with `key`; whether the editor took it.
fn command(driver: &mut Driver, key: &str) -> bool {
    driver.keystroke(Key::Character(key.into()), crate::test_support::command())
}

/// Whether `drawing` washes something in the selection's highlight.
fn highlighted(drawing: &DrawList, palette: Palette) -> bool {
    let mut highlight = DrawList::new();
    style::primary_highlight(palette, 1.0, &mut highlight, Default::default());
    let wash = highlight.0.iter().find_map(|command| match command {
        DrawCmd::Fill { brush, .. } => Some(brush),
        _ => None,
    });
    painted(&drawing.0)
        .any(|command| matches!(command, DrawCmd::Fill { brush, .. } if Some(brush) == wash))
}

#[test]
fn a_structural_copy_carries_a_picture_of_what_it_copied() {
    for zoom in [1.0, 2.0] {
        let (mut driver, path) = selecting_a_list(zoom);
        let (rect, value, palette) = (
            driver.rect(&path),
            driver.value(&path),
            driver.editor().palette,
        );
        assert!(highlighted(driver.painted(), palette));
        assert!(command(&mut driver, "c"));
        let mut region = None;
        driver.picture_copy(|picture| {
            assert!(!highlighted(&picture.drawing, palette));
            region = Some(picture.region);
            Ok(b"png".to_vec())
        });
        // The pixels the window draws the list on, at its scale.
        assert_eq!(region, Some(rect.expand()));
        assert_eq!(driver.copied_picture(), Some(&b"png"[..]));
        assert_eq!(driver.copied(), Some(value));
    }
}

#[test]
fn only_a_copy_of_structure_asks_for_a_picture() {
    let (mut driver, _) = selecting_a_list(1.0);
    assert!(command(&mut driver, "x"));
    driver.picture_copy(|_| panic!("a cut asks for no picture"));
    driver.select(&driver.element(&[], 0, &[]));
    assert!(command(&mut driver, "c"));
    driver.picture_copy(|_| panic!("a copy of text asks for no picture"));
    assert_eq!(driver.copied_picture(), None);
}

#[test]
fn a_picture_holds_only_what_is_in_view() {
    let mut driver = Driver::new(
        crate::editor_of(Value::list(
            (0..20).map(|line| text::value(&line.to_string())),
        )),
        Size::new(400.0, 120.0),
    );
    driver.select(&[]);
    let rect = driver.rect(&[]);
    assert!(command(&mut driver, "c"));
    let mut region = None;
    driver.picture_copy(|picture| {
        region = Some(picture.region);
        Ok(Vec::new())
    });
    let region = region.expect("a picture");
    assert_eq!(region.y0, rect.expand().y0);
    assert!(region.y1 <= 120.0 && region.y1 < rect.y1);
}

#[test]
fn a_crop_keeps_the_regions_pixels_row_by_row() {
    // Three by two pixels, their bytes counting up.
    let image: Vec<u8> = (0..24).collect();
    assert_eq!(
        crop(&image, 3, Rect::new(1.0, 0.0, 3.0, 2.0)),
        (4..12).chain(16..24).collect::<Vec<u8>>()
    );
}

#[test]
#[ignore = "needs a GPU"]
fn gpu_pictures_are_pngs_of_what_was_copied() {
    let gpu = Gpu::new();
    let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
    for zoom in [1.0, 2.0] {
        let (mut driver, path) = selecting_a_list(zoom);
        let rect = driver.rect(&path).expand();
        let paper = driver.editor().palette.paper.to_rgba8().to_u8_array();
        assert!(command(&mut driver, "c"));
        driver.picture_copy(|picture| {
            picture.png(
                &gpu,
                &mut compositor,
                &mut Resources::default(),
                &gpu.texture(
                    Driver::VIEWPORT.width as u32,
                    Driver::VIEWPORT.height as u32,
                ),
            )
        });
        let png = driver.copied_picture().expect("a picture");
        assert_eq!(
            crate::libraries::png::size(png),
            Some((rect.width() as u32, rect.height() as u32))
        );
        // It shows the list's text, not only paper.
        let image = crate::libraries::png::decode(png).unwrap();
        assert!(
            image
                .data
                .data()
                .chunks_exact(4)
                .any(|pixel| pixel != paper)
        );
    }
}
