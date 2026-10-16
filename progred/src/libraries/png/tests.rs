use super::*;
use crate::driver::Driver;
use gid::{Document, Step, new_cell_id};
use png::ColorType::{Grayscale, GrayscaleAlpha, Indexed, Rgb, Rgba};

type Encoder<'a> = png::Encoder<'a, &'a mut Vec<u8>>;

fn encode(width: u32, height: u32, setup: impl FnOnce(&mut Encoder), samples: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    setup(&mut encoder);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(samples).unwrap();
    writer.finish().unwrap();
    bytes
}

fn color(color: png::ColorType) -> impl FnOnce(&mut Encoder) {
    move |encoder| encoder.set_color(color)
}

fn pixels(image: ImageData) -> (u32, u32, Vec<u8>) {
    (image.width, image.height, image.data.data().to_vec())
}

#[test]
fn every_color_type_decodes_to_straight_rgba() {
    let palette = |encoder: &mut Encoder| {
        encoder.set_color(Indexed);
        encoder.set_palette(vec![9, 8, 7, 6, 5, 4]);
        encoder.set_trns(vec![255, 0]);
    };
    let wide = |encoder: &mut Encoder| {
        encoder.set_color(Rgb);
        encoder.set_depth(png::BitDepth::Sixteen);
    };
    for (bytes, expected) in [
        (
            encode(2, 1, color(Grayscale), &[0, 200]),
            [0, 0, 0, 255, 200, 200, 200, 255],
        ),
        (
            encode(2, 1, color(GrayscaleAlpha), &[10, 0, 20, 128]),
            [10, 10, 10, 0, 20, 20, 20, 128],
        ),
        (
            encode(2, 1, color(Rgb), &[1, 2, 3, 4, 5, 6]),
            [1, 2, 3, 255, 4, 5, 6, 255],
        ),
        (
            encode(2, 1, color(Rgba), &[1, 2, 3, 4, 5, 6, 7, 8]),
            [1, 2, 3, 4, 5, 6, 7, 8],
        ),
        (encode(2, 1, palette, &[1, 0]), [6, 5, 4, 0, 9, 8, 7, 255]),
        (
            encode(2, 1, wide, &[1, 99, 2, 99, 3, 99, 4, 99, 5, 99, 6, 99]),
            [1, 2, 3, 255, 4, 5, 6, 255],
        ),
    ] {
        assert_eq!(decode(&bytes).map(pixels), Some((2, 1, expected.to_vec())));
    }
}

#[test]
fn bytes_that_are_not_a_png_or_too_large_to_show_do_not_decode() {
    let pixel = encode(1, 1, color(Rgba), &[1, 2, 3, 4]);
    assert!(decode(&pixel).is_some());
    for bytes in [&[][..], b"not a png", &pixel[..pixel.len() - 20]] {
        assert!(decode(bytes).is_none());
    }

    // A bit a pixel compresses to almost nothing, but as RGBA it would
    // just exceed the limit. The header still gives its size.
    let (width, height) = (4097, 4096);
    let huge = encode(
        width,
        height,
        |encoder| {
            encoder.set_color(Grayscale);
            encoder.set_depth(png::BitDepth::One);
        },
        &vec![0; (width as usize).div_ceil(8) * height as usize],
    );
    assert_eq!(size(&huge), Some((width, height)));
    assert!(decode(&huge).is_none());
}

#[test]
fn images_show_at_their_own_size_until_they_shrink_to_fit() {
    assert_eq!(shown_size(100, 50), Size::new(100.0, 50.0));
    assert_eq!(shown_size(960, 480), Size::new(480.0, 240.0));
    assert_eq!(shown_size(240, 1920), Size::new(60.0, 480.0));
}

fn call(function: CellId, arguments: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    crate::libraries::test_evaluate(
        &::grap::call(::grap::ffi(function), arguments),
        |_| None,
        &functions(),
        100,
    )
    .result
}

#[test]
fn width_and_height_read_the_header() {
    let image = value(encode(3, 2, color(Rgb), &[0; 18]));
    assert_eq!(
        call(vocabulary::WIDTH, [(vocabulary::IMAGE, image.clone())]),
        crate::libraries::f64::value(3.0)
    );
    assert_eq!(
        call(vocabulary::HEIGHT, [(vocabulary::IMAGE, image)]),
        crate::libraries::f64::value(2.0)
    );
    assert_eq!(
        call(
            vocabulary::WIDTH,
            [(vocabulary::IMAGE, Value::from(vec![1]))]
        ),
        absent::with_reason(vocabulary::NOT_PNG)
    );
    assert_eq!(
        call(
            vocabulary::HEIGHT,
            [(vocabulary::IMAGE, value(b"not a png".to_vec()))]
        ),
        absent::with_reason(vocabulary::INVALID_PNG)
    );
    assert!(absent::is_absent(&call(vocabulary::WIDTH, [])));
}

fn driver(root: Value) -> Driver {
    Driver::new(
        crate::test_editor(Document {
            root: Some(root),
            cells: Cells::new(),
        }),
        Size::new(640.0, 480.0),
    )
}

#[test]
fn the_editor_shows_the_picture_and_the_record_around_it() {
    let rgba = [255, 0, 0, 255, 0, 0, 255, 128];
    let note = new_cell_id();
    let picture = Value::record(
        value(encode(2, 1, color(Rgba), &rgba))
            .as_record()
            .unwrap()
            .clone()
            .update(note, crate::libraries::text::value("red, then faint blue")),
    );
    let shown = driver(picture);
    assert_eq!(shown.images(), [rgba.to_vec()]);
    assert!(shown.drawn(&[Step::Key(note)]));

    let broken = driver(value(vec![1, 2, 3]));
    assert!(broken.images().is_empty());
    assert!(broken.drawn(&[Step::Key(vocabulary::PNG)]));
}

#[test]
fn the_library_names_its_vocabulary() {
    let library = library();
    assert_eq!(
        library.value(vocabulary::WIDTH).and_then(name::read),
        Some("png width")
    );
    assert_eq!(
        library.value(vocabulary::INVALID_PNG).and_then(name::read),
        Some("invalid png")
    );
}
