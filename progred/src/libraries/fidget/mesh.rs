//! An opt-in triangle viewport. Geometry and camera rendering are independent.

use super::*;
use crate::display::host::{Host, Layout, ProjectionInput};
use puri::mesh::{Geometry, Mesh, Surface, View};

#[cfg(feature = "cam-profile")]
pub(crate) mod performance;
#[cfg(test)]
#[path = "tests/mesh/mod.rs"]
pub(crate) mod tests;

const DEFAULT_DEPTH: u64 = 6;

fn depth(value: Option<&Value>) -> Option<u8> {
    let depth = value
        .map(crate::libraries::u64::read)
        .unwrap_or(Some(DEFAULT_DEPTH))?;
    (1..=8).contains(&depth).then_some(depth as u8)
}

pub(crate) fn preview(
    context: &mut ::grap::Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    let requested = evaluated(context, call, environment, vocabulary::MESH_DEPTH)?;
    Ok(match depth(requested.as_ref()) {
        None => ::grap::absent::value(vocabulary::INVALID_MESH_DEPTH),
        Some(depth) => {
            let value = volume_preview_function(
                context,
                call,
                environment,
                vocabulary::kind::MESH_PREVIEW,
            )?;
            match of_kind(&value, vocabulary::kind::MESH_PREVIEW) {
                Some(fields) => Value::Record(fields.with(
                    vocabulary::MESH_DEPTH,
                    crate::libraries::u64::value(u64::from(depth)),
                )),
                None => value,
            }
        }
    })
}

fn generate(preview: &VolumePreview, depth: u8) -> Option<Mesh> {
    let mut geometry = Geometry::default();
    preview
        .shape
        .append(&mut geometry, depth, &incremental::Cancellation::default())
        .ok()
        .flatten()
        .map(|()| geometry.into())
}

impl Shape {
    pub fn append(
        &self,
        out: &mut Geometry,
        depth: u8,
        cancellation: &incremental::Cancellation,
    ) -> Result<Option<()>, incremental::Error> {
        volume::mesh::append(out, &self.objects, self.min, self.max, depth, cancellation)
    }
}

fn view(preview: &VolumePreview, camera: Camera, pixels: PixelRenderSize) -> Option<View> {
    let (center, radius) = framing(preview.shape.min, preview.shape.max);
    (radius.is_finite() && radius > 0.0).then(|| {
        // Match the implicit view's vertical framing, independent of pane width.
        let vertical_scale = camera.zoom / radius;
        View {
            model_to_view: orientation(camera).inverse().to_homogeneous()
                * Translation3::from(-center).to_homogeneous(),
            projection: [
                vertical_scale * pixels.height() as f32 / pixels.width() as f32,
                vertical_scale,
                -0.5 / radius,
                0.5,
            ],
            width: pixels.width(),
            height: pixels.height(),
        }
    })
}

/// A preview's mesh, made again only when its shape or depth changes, not as
/// the camera turns.
struct Meshed {
    request: incremental::Input<(VolumePreview, u8)>,
    mesh: incremental::Memo<Option<Mesh>>,
}

impl Meshed {
    fn new(runtime: &incremental::Runtime, request: (VolumePreview, u8)) -> Self {
        let request = runtime.input(request);
        Self {
            mesh: runtime.memo_by(
                {
                    let request = request.clone();
                    move |read| {
                        let (preview, depth) = &*request.read(read);
                        Ok(generate(preview, *depth))
                    }
                },
                // A new mesh is a new mesh; comparing vertex buffers costs more.
                |_, _| false,
            ),
            request,
        }
    }
}

pub(super) fn display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let request = read(input.value?.as_value())?;
    let state = input.state.cloned();
    let scale_factor = input.scale_factor;
    Some(interactive_volume(
        Layout::program(Rc::new(move |context, build| {
            let computations = context.viewing().computations();
            let meshed = context.site().memo(computations, || {
                Meshed::new(&computations.runtime, request.clone())
            });
            meshed.request.set(request.clone());
            let drawing = match computations.runtime.read(&meshed.mesh) {
                Ok(mesh) => mesh
                    .as_ref()
                    .as_ref()
                    .and_then(|mesh| drawing(mesh, &request.0, state.as_ref(), scale_factor)),
                Err(_) => None,
            };
            drawing
                .unwrap_or_else(|| {
                    crate::libraries::presentation::result_layout(::grap::absent::value(
                        vocabulary::INVALID_FIELD,
                    ))
                })
                .measure(context, build)
        })),
        input,
    ))
}

pub(crate) fn read(value: &Value) -> Option<(VolumePreview, u8)> {
    let fields = of_kind(value, vocabulary::kind::MESH_PREVIEW)?;
    Some((
        volume_preview_fields(fields)?,
        depth(fields.get(&vocabulary::MESH_DEPTH))?,
    ))
}

pub(crate) fn drawing<W: Host>(
    geometry: &Mesh,
    preview: &VolumePreview,
    state: Option<&Value>,
    scale_factor: f64,
) -> Option<Layout<W>> {
    drawing_surface(geometry, None, preview, state, scale_factor)
}

/// `preview`'s box, blank but for `lines` of dim text: what a preview shows
/// where it can't draw its picture.
pub(crate) fn note<W: Host>(preview: &VolumePreview, lines: &[&'static str]) -> Layout<W> {
    let size = preview.size;
    let blank = Layout::widget(Rc::new(move |context| {
        let scale = context.viewing().scale();
        widget::paint(
            widget::Extent {
                width: size.width * scale,
                ascent: size.height / 2.0 * scale,
                descent: size.height / 2.0 * scale,
            },
            |_, _| {},
        )
    }));
    let text = lines
        .iter()
        .map(|line| crate::display::center(crate::display::dim(*line)));
    crate::display::overlay([
        blank,
        crate::display::center(crate::display::col(0, 2.0, text)),
    ])
}

pub(crate) fn drawing_surface<W: Host>(
    geometry: &Mesh,
    surface: Option<Surface>,
    preview: &VolumePreview,
    state: Option<&Value>,
    scale_factor: f64,
) -> Option<Layout<W>> {
    let pixels = raster_size(preview.size, scale_factor)?;
    let scene = puri::mesh::Scene {
        geometry: geometry.clone(),
        view: view(preview, camera(state), pixels)?,
        surface,
    };
    let size = preview.size;
    let image_transform = Affine::scale_non_uniform(
        size.width / f64::from(pixels.width()),
        size.height / f64::from(pixels.height()),
    );
    Some(Layout::widget(Rc::new(move |context| {
        let scale = context.viewing().scale();
        let scene = scene.clone();
        widget::paint(
            widget::Extent {
                width: size.width * scale,
                ascent: size.height / 2.0 * scale,
                descent: size.height / 2.0 * scale,
            },
            move |canvas, placement| {
                let transform = Affine::translate((placement.rect.x0, placement.rect.y0))
                    * Affine::scale(scale);
                canvas.draw_mesh(scene, transform * image_transform);
            },
        )
    })))
}
