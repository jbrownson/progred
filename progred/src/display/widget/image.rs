//! A raster image drawn over a logical size.

use crate::display::{Layout, widget};
use kurbo::{Affine, Size};
use peniko::{ImageAlphaType, ImageData, ImageFormat};
use std::rc::Rc;

/// An image of straight-alpha RGBA8 pixels.
pub fn rgba_image(width: u32, height: u32, rgba: Vec<u8>) -> ImageData {
    ImageData {
        data: rgba.into(),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width,
        height,
    }
}

/// `image` stretched over `size`, which is in logical units.
pub fn image<World: 'static, Hover: 'static>(size: Size, image: ImageData) -> Layout<World, Hover> {
    let image_transform = Affine::scale_non_uniform(
        size.width / f64::from(image.width),
        size.height / f64::from(image.height),
    );
    Layout::widget(Rc::new(move |context| {
        let scale = context.toolkit.styles().scale;
        let image = image.clone();
        widget::paint(
            widget::Extent {
                width: size.width * scale,
                ascent: size.height / 2.0 * scale,
                descent: size.height / 2.0 * scale,
            },
            move |canvas, placement| {
                let transform = Affine::translate((placement.rect.x0, placement.rect.y0))
                    * Affine::scale(scale);
                canvas.draw_image(image, transform * image_transform);
            },
        )
    }))
}
