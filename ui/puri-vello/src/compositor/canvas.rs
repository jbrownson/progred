use crate::VelloCanvas;
use puri::draw::{CanvasSink, GlyphRun, Shape};
use vello::{
    Scene,
    kurbo::Affine,
    peniko::{Brush, ImageData},
};

pub enum Layer {
    Mesh(puri::mesh::Scene, Affine),
    Vector(Scene),
    Image(ImageData, Affine),
    Texture(vello::wgpu::Texture, Affine),
    Clip(Shape, Affine, Vec<Layer>),
}

#[derive(Default)]
pub struct SplitCanvas {
    scene: Scene,
    dirty: bool,
    layers: Vec<Layer>,
}

impl SplitCanvas {
    fn flush(&mut self) {
        if self.dirty {
            self.layers
                .push(Layer::Vector(std::mem::take(&mut self.scene)));
            self.dirty = false;
        }
    }

    pub fn finish(mut self) -> Vec<Layer> {
        self.flush();
        self.layers
    }
}

impl CanvasSink for SplitCanvas {
    fn draw_mesh(&mut self, scene: puri::mesh::Scene, transform: Affine) {
        if scene.view.width > 0 && scene.view.height > 0 && transform.determinant() != 0.0 {
            self.flush();
            self.layers.push(Layer::Mesh(scene, transform));
        }
    }
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        if image.width > 0 && image.height > 0 && transform.determinant() != 0.0 {
            self.flush();
            self.layers.push(Layer::Image(image, transform));
        }
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        VelloCanvas(&mut self.scene).fill_shape(shape, brush, transform);
        self.dirty = true;
    }

    fn stroke_shape(
        &mut self,
        shape: Shape,
        style: vello::kurbo::Stroke,
        brush: Brush,
        transform: Affine,
    ) {
        VelloCanvas(&mut self.scene).stroke_shape(shape, style, brush, transform);
        self.dirty = true;
    }

    fn draw_glyphs(&mut self, run: GlyphRun) {
        VelloCanvas(&mut self.scene).draw_glyphs(run);
        self.dirty = true;
    }

    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn CanvasSink) + '_>,
    ) {
        let mut child = Self::default();
        content(&mut child);
        let layers = child.finish();
        if layers.is_empty() {
        } else if layers.iter().all(|layer| matches!(layer, Layer::Vector(_))) {
            VelloCanvas(&mut self.scene).push_clip(&shape, transform);
            for layer in layers {
                if let Layer::Vector(scene) = layer {
                    self.scene.append(&scene, None);
                }
            }
            VelloCanvas(&mut self.scene).pop_clip();
            self.dirty = true;
        } else {
            self.flush();
            self.layers.push(Layer::Clip(shape, transform, layers));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use puri::draw::Canvas;
    use vello::{
        kurbo::Rect,
        peniko::{Color, ImageAlphaType, ImageFormat},
    };

    fn image() -> ImageData {
        ImageData {
            data: vec![255; 4].into(),
            width: 1,
            height: 1,
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
        }
    }

    #[test]
    fn images_split_only_nonempty_vector_runs() {
        let mut canvas = SplitCanvas::default();
        canvas.image(image(), Affine::IDENTITY);
        canvas.image(image(), Affine::IDENTITY);
        canvas.fill(
            Rect::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            Affine::IDENTITY,
        );
        canvas.image(image(), Affine::IDENTITY);
        assert!(matches!(
            canvas.finish().as_slice(),
            [
                Layer::Image(..),
                Layer::Image(..),
                Layer::Vector(_),
                Layer::Image(..)
            ]
        ));
    }

    #[test]
    fn vector_only_clips_remain_one_vector_run() {
        let mut canvas = SplitCanvas::default();
        canvas.clip(Rect::new(0.0, 0.0, 4.0, 4.0), Affine::IDENTITY, |canvas| {
            canvas.fill(
                Rect::new(0.0, 0.0, 2.0, 2.0),
                Color::WHITE,
                Affine::IDENTITY,
            );
        });
        canvas.fill(
            Rect::new(0.0, 0.0, 1.0, 1.0),
            Color::BLACK,
            Affine::IDENTITY,
        );
        assert!(matches!(canvas.finish().as_slice(), [Layer::Vector(_)]));
    }

    #[test]
    fn mixed_clips_preserve_the_scope_and_paint_order() {
        let mut canvas = SplitCanvas::default();
        canvas.clip(Rect::new(0.0, 0.0, 4.0, 4.0), Affine::IDENTITY, |canvas| {
            canvas.fill(
                Rect::new(0.0, 0.0, 2.0, 2.0),
                Color::WHITE,
                Affine::IDENTITY,
            );
            canvas.image(image(), Affine::IDENTITY);
        });
        let layers = canvas.finish();
        assert!(matches!(layers.as_slice(), [Layer::Clip(_, _, children)]
            if matches!(children.as_slice(), [Layer::Vector(_), Layer::Image(..)])));
    }

    #[test]
    fn empty_clip_emits_nothing() {
        let mut canvas = SplitCanvas::default();
        canvas.clip(Rect::ZERO, Affine::IDENTITY, |_| {});
        assert!(canvas.finish().is_empty());
    }
}
