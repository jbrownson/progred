//! Worker-local assembly of finished tiles, with bounded snapshot publication.
use super::*;
use fidget_engine::raster::voxel::SceneTile;
use std::sync::Mutex;

struct Assembly {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    depth: Vec<f32>,
    depth_count: u32,
    partial: bool,
    completed: usize,
}

struct Publication<'a> {
    assembly: Assembly,
    publish: &'a mut (dyn FnMut(DepthImage) -> Result<(), incremental::Error> + Send),
    error: Option<incremental::Error>,
    last: web_time::Instant,
}

impl Publication<'_> {
    fn maybe_publish(&mut self) {
        let due = self.last.elapsed() >= std::time::Duration::from_millis(100);
        if due && self.assembly.completed < self.assembly.width * self.assembly.height {
            self.error = (self.publish)(self.assembly.snapshot()).err();
            self.last = web_time::Instant::now();
        }
    }
}

impl Assembly {
    fn new(width: usize, height: usize, depth_count: u32, previous: Option<&DepthImage>) -> Self {
        let mut pixels = vec![0; width * height * 4];
        let mut depth = vec![-1.0; width * height];
        if let Some(previous) = previous {
            let image = &previous.image;
            for y in 0..height {
                for x in 0..width {
                    let src = ((y * image.height as usize / height) * image.width as usize
                        + x * image.width as usize / width)
                        * 4;
                    let dst = (y * width + x) * 4;
                    pixels[dst..dst + 4].copy_from_slice(&image.data.data()[src..src + 4]);
                    depth[dst / 4] = previous.depth[src / 4];
                }
            }
        }
        Self {
            width,
            height,
            pixels,
            depth,
            depth_count,
            partial: previous.is_none(),
            completed: 0,
        }
    }

    fn put(
        &mut self,
        tile: SceneTile<'_>,
        shade: &impl Fn(GeometryPixel, [u8; 3]) -> [u8; 4],
        colors: &[[u8; 3]],
    ) {
        for y in 0..tile.size[1] {
            for x in 0..tile.size[0] {
                let pixel = tile.pixels[y * tile.stride + x];
                let color = pixel.object.map_or([255; 3], |i| colors[i]);
                let dst = ((tile.origin[1] + y) * self.width + tile.origin[0] + x) * 4;
                self.pixels[dst..dst + 4].copy_from_slice(&shade(pixel.geometry, color));
                // Fidget stores the front boundary of the occupied voxel, with
                // larger Z nearer the camera; zero denotes an empty ray.
                self.depth[dst / 4] = 1.0 - pixel.geometry.depth as f32 / self.depth_count as f32;
            }
        }
        self.completed += tile.size[0] * tile.size[1];
    }

    fn snapshot(&self) -> DepthImage {
        DepthImage {
            image: rgba_image(self.width as u32, self.height as u32, self.pixels.clone()),
            depth: self.depth.clone().into(),
            partial: self.partial,
        }
    }
}

pub(super) fn render(
    scene: &SoftwareScene,
    view: &VolumeView,
    previous: Option<&DepthImage>,
    cancel: &incremental::Cancellation,
    publish: &mut (dyn FnMut(DepthImage) -> Result<(), incremental::Error> + Send),
    progress: Option<&(dyn Fn(Progress) + Sync)>,
) -> Result<Option<DepthImage>, incremental::Error> {
    let assembly = Assembly::new(
        view.size.width() as usize,
        view.size.height() as usize,
        view.size.depth(),
        previous,
    );
    let shade = shading(&VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    });
    // Publication is serialized with assembly: parallel workers cannot publish
    // an older snapshot after a newer one. No UI or editor state crosses here.
    let state = Mutex::new(Publication {
        assembly,
        publish,
        error: None,
        last: web_time::Instant::now(),
    });
    let tile_ready = |tile: SceneTile<'_>| {
        let mut state = state.lock().unwrap();
        if cancel.check().is_err() || state.error.is_some() {
            return;
        }
        state.assembly.put(tile, &shade, &scene.colors);
        state.maybe_publish();
        if state.error.is_some() {
            cancel.cancel();
        }
    };
    let output = scene.render_tiles(view, cancel, progress, &tile_ready);
    let state = state.into_inner().unwrap();
    if let Some(error) = state.error {
        return Err(error);
    }
    cancel.check()?;
    Ok(output.map(|()| DepthImage {
        image: rgba_image(view.size.width(), view.size.height(), state.assembly.pixels),
        depth: state.assembly.depth.into(),
        partial: false,
    }))
}

#[cfg(test)]
#[path = "tests/partial.rs"]
mod tests;
