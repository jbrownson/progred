//! PNG images as an open record convention: `{png: bytes}` holds an encoded
//! PNG, and the projection shows the picture.

use crate::display::{Layout, ProjectionInput};
use crate::libraries::{Library, absent, name};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::{CellId, Cells, Value};
use puri::{ImageAlphaType, ImageData, ImageFormat, Size};

pub const ID: CellId = CellId::from_u128(0x40322c0867ad9170420d563b8b15aa55);

pub mod vocabulary {
    use gid::CellId;

    pub const PNG: CellId = CellId::from_u128(0x2ad76f2202896e62c4d974df656f838b);
    pub const IMAGE: CellId = CellId::from_u128(0x094b37f921f94ebd9f079aed6136eb8d);
    pub const WIDTH: CellId = CellId::from_u128(0x4a723bde93cf456a94fa29201e84043a);
    pub const HEIGHT: CellId = CellId::from_u128(0x4fabf4dead3077a70076b17afe82ec2d);
    pub const NOT_PNG: CellId = CellId::from_u128(0x020931abaa8e8cf22c18deca1263c779);
    pub const INVALID_PNG: CellId = CellId::from_u128(0xf8f424de3d4ef22c36c3071ddb454a90);
}

/// The most memory decoding may use, and the decoded RGBA pixels may take:
/// the png crate's default, which a 5K screenshot fits in.
const LIMIT: usize = 64 << 20;

/// The longest side shown at the image's own size; larger images shrink to fit.
const SHOWN: f64 = 480.0;

#[cfg(any(test, feature = "control"))]
pub fn value(bytes: Vec<u8>) -> Value {
    Value::record([(vocabulary::PNG, Value::from(bytes))])
}

/// Eight-bit straight-alpha RGBA pixels as a PNG.
#[cfg(any(test, feature = "control"))]
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(bytes)
}

pub fn read(value: &Value) -> Option<&[u8]> {
    value.as_record()?.get(&vocabulary::PNG)?.as_blob()
}

fn reader(bytes: &[u8]) -> Option<png::Reader<std::io::Cursor<&[u8]>>> {
    let mut decoder =
        png::Decoder::new_with_limits(std::io::Cursor::new(bytes), png::Limits { bytes: LIMIT });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    decoder.read_info().ok()
}

/// Width and height in pixels, from the header alone.
pub fn size(bytes: &[u8]) -> Option<(u32, u32)> {
    reader(bytes).map(|reader| reader.info().size())
}

/// The image's pixels as straight-alpha RGBA, or `None` when the bytes
/// aren't a PNG or the pixels would exceed the limit.
pub fn decode(bytes: &[u8]) -> Option<ImageData> {
    let mut reader = reader(bytes).filter(|reader| fits(reader.info().size()))?;
    let mut samples = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut samples).ok()?;
    samples.truncate(frame.buffer_size());
    Some(ImageData {
        data: rgba(frame.color_type, samples)?.into(),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width: frame.width,
        height: frame.height,
    })
}

fn fits((width, height): (u32, u32)) -> bool {
    u64::from(width) * u64::from(height) * 4 <= LIMIT as u64
}

/// Decoded 8-bit samples as RGBA. Palettes are already expanded.
fn rgba(color: png::ColorType, samples: Vec<u8>) -> Option<Vec<u8>> {
    match color {
        png::ColorType::Rgba => Some(samples),
        png::ColorType::Rgb => Some(
            samples
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&[red, green, blue]| [red, green, blue, u8::MAX])
                .collect(),
        ),
        png::ColorType::GrayscaleAlpha => Some(
            samples
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[gray, alpha]| [gray, gray, gray, alpha])
                .collect(),
        ),
        png::ColorType::Grayscale => Some(
            samples
                .iter()
                .flat_map(|&gray| [gray, gray, gray, u8::MAX])
                .collect(),
        ),
        png::ColorType::Indexed => None,
    }
}

/// The image's own pixel size in logical units, shrunk to fit within
/// `SHOWN` on its longer side.
fn shown_size(width: u32, height: u32) -> Size {
    let size = Size::new(f64::from(width), f64::from(height));
    size * (SHOWN / size.width.max(size.height)).min(1.0)
}

/// A PNG reads as its picture, then any other fields. Bytes that don't
/// decode decline, so the record shows as ordinary data.
pub fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, ::grap::RuntimeValue>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let bytes = input.value?.field(vocabulary::PNG)?;
    let image = decode(bytes.as_blob()?)?;
    Some(crate::display::structure::with_extra_fields(
        input,
        |key| key == vocabulary::PNG,
        crate::display::widget::image::image(shown_size(image.width, image.height), image),
    ))
}

fn dimension(pick: fn((u32, u32)) -> u32) -> ForeignFunction {
    ForeignFunction::from_value(move |context, call, environment| {
        Ok(match context.field(call, vocabulary::IMAGE) {
            None => context.missing_argument(vocabulary::IMAGE),
            Some(image) => match read(&context.eval_to_value(image, environment)?) {
                None => absent::with_reason(vocabulary::NOT_PNG),
                Some(bytes) => size(bytes).map_or_else(
                    || absent::with_reason(vocabulary::INVALID_PNG),
                    |size| crate::libraries::f64::value(f64::from(pick(size))),
                ),
            },
        })
    })
    .parameters([vocabulary::IMAGE])
    .tracked()
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(vocabulary::WIDTH, dimension(|(width, _)| width))
        .register(vocabulary::HEIGHT, dimension(|(_, height)| height))
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::PNG, "png"),
        (vocabulary::IMAGE, "image"),
        (vocabulary::WIDTH, "png width"),
        (vocabulary::HEIGHT, "png height"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    for (cell, reason) in [
        (vocabulary::NOT_PNG, "not a png"),
        (vocabulary::INVALID_PNG, "invalid png"),
    ] {
        cells.set_value(cell, absent::named_reason(reason));
    }
    Library::named(
        ID,
        "png",
        crate::libraries::Definitions::from_parts(cells, functions()),
    )
    .with_projection(crate::display::runtime_partial(display))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::Driver;
    use gid::{Document, Step, new_cell_id};
    use png::ColorType::{Grayscale, GrayscaleAlpha, Indexed, Rgb, Rgba};

    type Encoder<'a> = png::Encoder<'a, &'a mut Vec<u8>>;

    fn encode(
        width: u32,
        height: u32,
        setup: impl FnOnce(&mut Encoder),
        samples: &[u8],
    ) -> Vec<u8> {
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
}
