//! CPU interpretation used by geometry tests.
use super::*;

pub(super) fn render(geometry: &Geometry, view: &View) -> Option<Vec<u8>> {
    render_surface(geometry, view, None)
}

pub(super) fn render_surface(
    geometry: &Geometry,
    view: &View,
    surface: Option<Surface>,
) -> Option<Vec<u8>> {
    let image = puri::mesh::Scene {
        geometry: Mesh::new(geometry.clone()),
        view: view.clone(),
        surface,
    }
    .rasterize()?;
    Some(image.data.data().to_vec())
}
