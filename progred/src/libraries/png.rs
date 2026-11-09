//! PNG images as an open record convention: `{png: bytes}` holds an encoded
//! PNG, and the projection shows the picture.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, name};
use ::grap::arguments::Parse;
use ::grap::{ForeignFunction, ForeignFunctions, RuntimeValue};
use gid::{CellId, Value};
use kurbo::Size;
use peniko::ImageData;

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
#[cfg(any(target_os = "macos", target_os = "linux"))]
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
    Some(crate::display::widget::image::rgba_image(
        frame.width,
        frame.height,
        rgba(frame.color_type, samples)?,
    ))
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
pub fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let bytes = input.value?.field(vocabulary::PNG)?;
    let image = decode(bytes.as_blob()?)?;
    Some(crate::display::structure::Fields::default().after(
        input,
        |key| key == vocabulary::PNG,
        crate::display::widget::image::image(shown_size(image.width, image.height), image),
    ))
}

/// An argument that's a png.
struct Png(Value);

impl Png {
    fn bytes(&self) -> &[u8] {
        read(&self.0).unwrap_or_default()
    }
}

impl Parse for Png {
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        let png = value.to_value();
        if read(&png).is_some() {
            Ok(Self(png))
        } else {
            Err(::grap::arguments::not(vocabulary::NOT_PNG, value))
        }
    }
}

fn dimension(pick: fn((u32, u32)) -> u32) -> ForeignFunction<'static> {
    ForeignFunction::with_arguments([vocabulary::IMAGE], move |_, (image,): (Png,)| {
        size(image.bytes()).map_or_else(
            || ::grap::absent::value(vocabulary::INVALID_PNG),
            |size| crate::libraries::f64::value(f64::from(pick(size))),
        )
    })
    .cacheable()
}

fn functions() -> ForeignFunctions<'static> {
    ForeignFunctions::default()
        .register(vocabulary::WIDTH, dimension(|(width, _)| width))
        .register(vocabulary::HEIGHT, dimension(|(_, height)| height))
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "png",
        [
            (vocabulary::PNG, "png"),
            (vocabulary::IMAGE, "image"),
            (vocabulary::WIDTH, "png width"),
            (vocabulary::HEIGHT, "png height"),
            (vocabulary::NOT_PNG, "not a png"),
            (vocabulary::INVALID_PNG, "invalid png"),
        ],
        functions(),
    )
    .with_projection(crate::display::runtime_partial(display))
}

#[cfg(test)]
#[path = "tests/png.rs"]
mod tests;
