//! Whole-pass reference for pixel and timing comparisons, without tile publications.
use super::*;

impl Request {
    /// Publish up to native image resolution, then optionally refine depth in one final pass.
    /// Completed rasters are independent; only scene compilation is shared within the job.
    /// Uses software evaluation explicitly: the GPU VM cannot execute spilling tapes.
    pub fn render_software_progressive(
        &self,
        first_max_edge: u32,
        final_depth_multiplier: u32,
        cancel: &incremental::Cancellation,
        publish: &mut dyn FnMut(ImageData) -> Result<(), incremental::Error>,
        progress: Option<&(dyn Fn(Progress) + Sync)>,
    ) -> Result<Option<ImageData>, incremental::Error> {
        cancel.check()?;
        assert!(first_max_edge > 0);
        let Some(views) = self.refinements(first_max_edge, final_depth_multiplier) else {
            return Ok(None);
        };
        let scene = SoftwareScene::new(&self.preview.objects, cancel);
        cancel.check()?;
        let Some(scene) = scene else { return Ok(None) };
        let mut views = views.into_iter().peekable();
        while let Some(view) = views.next() {
            cancel.check()?;
            let rgba = scene.render_with_progress(&view, cancel, progress);
            cancel.check()?;
            let Some(rgba) = rgba else { return Ok(None) };
            let image = ImageData {
                data: rgba.into(),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: view.size.width(),
                height: view.size.height(),
            };
            if views.peek().is_none() {
                return Ok(Some(image));
            }
            publish(image)?;
        }
        unreachable!("the native resolution is always present")
    }
}
