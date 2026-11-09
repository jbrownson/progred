//! Pictures of what a window draws, rendered offscreen and read back as PNGs:
//! the picture a structural copy puts beside its text for other apps, and a
//! control session's screenshots.

use crate::Editor;
use crate::frame::Dispatch;
use crate::occurrence::Trail;
use gid::Value;
use kurbo::{Rect, Size};
use puri::draw::DrawList;
use puri_vello::capture::Gpu;
use puri_vello::compositor::{Compositor, Resources, SplitCanvas};
use vello::wgpu;

/// A structural copy waiting for its picture: the value copied, and the
/// occurrence it was drawn as.
pub(crate) struct Copied {
    value: Value,
    drawn: Trail,
}

impl Copied {
    pub(crate) fn new(value: &Value, drawn: &Trail) -> Self {
        Self {
            value: value.clone(),
            drawn: drawn.clone(),
        }
    }
}

/// The part of a window's drawing in `region`, whole pixels of the window,
/// over its paper.
pub(crate) struct Picture {
    pub(crate) drawing: DrawList,
    pub(crate) paper: peniko::Color,
    pub(crate) region: Rect,
}

impl Editor {
    /// Puts beside the last structural copy a picture of what it copied, as
    /// this window draws it at `scale` in `viewport`, with `render` making
    /// the PNG.
    pub(crate) fn picture_copy(
        &mut self,
        scale: f64,
        viewport: Size,
        render: impl FnOnce(&Picture) -> Result<Vec<u8>, String>,
    ) {
        if let Some(copied) = self.unpictured.take()
            && let Some(picture) = picture(self, &copied.drawn, scale, viewport)
        {
            match render(&picture) {
                Ok(png) => {
                    let (text, _) = crate::clipboard::spelling(&copied.value);
                    self.text_clipboard
                        .copy(text, Some(&copied.value), Some(png));
                }
                Err(error) => eprintln!("failed to picture a copy: {error}"),
            }
        }
    }
}

/// What `editor`'s window draws as `drawn`, at `scale` in `viewport`, with
/// nothing selected and no pointer over it, so no highlight marks it: as
/// much of it as is in view, or none when none is.
fn picture(editor: &mut Editor, drawn: &Trail, scale: f64, viewport: Size) -> Option<Picture> {
    let frame = editor.unmarked_frame(scale, viewport);
    shown(&frame.dispatch, drawn, viewport).map(|region| {
        let mut drawing = DrawList::new();
        puri::frame::render(frame.renders, &mut drawing);
        Picture {
            drawing,
            paper: editor.palette.paper,
            region,
        }
    })
}

/// The whole pixels of `viewport` that hold what `dispatch`'s frame draws
/// as `drawn`, within its area.
fn shown(dispatch: &Dispatch, drawn: &Trail, viewport: Size) -> Option<Rect> {
    dispatch
        .landmarks
        .iter()
        .find(|landmark| landmark.path == *drawn)
        .map(|landmark| {
            dispatch
                .view_regions
                .iter()
                .filter(|region| Some(&region.area) == landmark.area.as_ref())
                .fold(
                    landmark.rect.intersect(viewport.to_rect()),
                    |rect, region| rect.intersect(region.rect),
                )
        })
        .filter(|rect| !rect.is_zero_area())
        .map(Rect::expand)
}

impl Picture {
    /// The picture as a PNG, composited onto `target`, a texture the size of
    /// the window it was drawn for.
    pub(crate) fn png(
        &self,
        gpu: &Gpu,
        compositor: &mut Compositor,
        resources: &mut Resources,
        target: &wgpu::Texture,
    ) -> Result<Vec<u8>, String> {
        let mut canvas = SplitCanvas::default();
        puri::draw::replay(&self.drawing, &mut canvas);
        let output = compositor
            .render(
                &gpu.device,
                &gpu.queue,
                &canvas.finish(),
                resources,
                target,
                self.paper,
            )
            .map_err(|error| error.to_string())?;
        crate::libraries::png::encode(
            self.region.width() as u32,
            self.region.height() as u32,
            &crop(
                &gpu.read(&output.texture),
                output.texture.width(),
                self.region,
            ),
        )
        .map_err(|error| error.to_string())
    }
}

/// `region` of an image `width` pixels wide, both as rows of RGBA pixels.
fn crop(rgba: &[u8], width: u32, region: Rect) -> Vec<u8> {
    let (left, right) = (region.x0 as usize * 4, region.x1 as usize * 4);
    rgba.chunks_exact(width as usize * 4)
        .skip(region.y0 as usize)
        .take(region.height() as usize)
        .flat_map(|row| &row[left..right])
        .copied()
        .collect()
}

#[cfg(test)]
#[path = "tests/picture.rs"]
mod tests;
