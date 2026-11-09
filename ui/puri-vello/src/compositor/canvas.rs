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
#[path = "tests/canvas.rs"]
mod tests;
