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
    ) -> Result<Option<ImageData>, incremental::Error> {
        cancel.check()?;
        assert!(first_max_edge > 0);
        match self
            .view()
            .refinements(first_max_edge, final_depth_multiplier)
        {
            Some(views) => match SoftwareScene::new(&self.preview.objects, cancel)? {
                Some(scene) => whole_passes(&scene, &views, cancel, publish),
                None => Ok(None),
            },
            None => Ok(None),
        }
    }
}

/// Render each of `views` whole, and publish every image but the last, which
/// is the result.
fn whole_passes(
    scene: &SoftwareScene,
    views: &[VolumeView],
    cancel: &incremental::Cancellation,
    publish: &mut dyn FnMut(ImageData) -> Result<(), incremental::Error>,
) -> Result<Option<ImageData>, incremental::Error> {
    match views.split_first() {
        Some((view, finer)) => {
            cancel.check()?;
            let rgba = scene.render(view, cancel);
            cancel.check()?;
            match rgba.map(|rgba| ImageData {
                data: rgba.into(),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: view.size.width(),
                height: view.size.height(),
            }) {
                Some(image) if !finer.is_empty() => {
                    publish(image)?;
                    whole_passes(scene, finer, cancel, publish)
                }
                image => Ok(image),
            }
        }
        None => Ok(None),
    }
}
