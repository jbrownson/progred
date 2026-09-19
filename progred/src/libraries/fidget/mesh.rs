//! An opt-in triangle viewport. Geometry and camera rendering are independent.

use super::*;
use fidget_engine::mesh::{Octree, Settings};

#[cfg(test)]
mod cpu;
#[cfg(feature = "cam-profile")]
pub(crate) mod performance;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod gpu;
#[cfg(test)]
mod tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use gpu::{Renderer, raster, raster_surface};

const DEFAULT_DEPTH: u64 = 6;

fn depth(value: Option<&Value>) -> Option<u8> {
    let depth = value
        .map(crate::libraries::u64::read)
        .unwrap_or(Some(DEFAULT_DEPTH))?;
    (1..=8).contains(&depth).then_some(depth as u8)
}

pub(crate) fn preview(
    context: &mut ::grap::Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    let requested = evaluated(context, call, environment, vocabulary::MESH_DEPTH)?;
    let Some(depth) = depth(requested.as_ref()) else {
        return Ok(absent::with_reason(vocabulary::INVALID_MESH_DEPTH));
    };
    let value = volume_preview_function(context, call, environment, vocabulary::PREVIEW_MESH)?;
    Ok(
        if let Some(fields) = value
            .as_record()
            .and_then(|r| r.get(&vocabulary::PREVIEW_MESH))
            .and_then(Value::as_record)
        {
            node(
                vocabulary::PREVIEW_MESH,
                Value::record(fields.iter().map(|(k, v)| (*k, v.clone())).chain([(
                    vocabulary::MESH_DEPTH,
                    crate::libraries::u64::value(u64::from(depth)),
                )])),
            )
        } else {
            value
        },
    )
}

pub(crate) use puri::mesh::{Geometry, Mesh, Normal, Surface, Vertex, View};

fn generate(preview: &VolumePreview, depth: u8) -> Option<Mesh> {
    let mut geometry = Geometry::default();
    append(&mut geometry, preview, depth)?;
    Some(geometry.into())
}

pub(crate) fn append(out: &mut Geometry, preview: &VolumePreview, depth: u8) -> Option<()> {
    Shape::from(preview).append(out, depth)
}

#[derive(Clone, PartialEq)]
pub(crate) struct Shape {
    pub objects: Vec<SceneObject>,
    min: Vector3<f32>,
    max: Vector3<f32>,
}

impl From<&VolumePreview> for Shape {
    fn from(preview: &VolumePreview) -> Self {
        Self {
            objects: preview.objects.clone(),
            min: preview.min,
            max: preview.max,
        }
    }
}

impl Shape {
    pub fn append(&self, out: &mut Geometry, depth: u8) -> Option<()> {
        self.append_cancellable(out, depth, &incremental::Cancellation::default())
            .ok()?
    }

    pub fn append_cancellable(
        &self,
        out: &mut Geometry,
        depth: u8,
        cancellation: &incremental::Cancellation,
    ) -> Result<Option<()>, incremental::Error> {
        cancellation.check()?;
        let settings = Settings {
            depth,
            cancel: fidget_engine::render::CancelToken::from_shared_flag(
                cancellation.shared_flag().clone(),
            ),
            world_to_model: Translation3::from((self.min + self.max) / 2.0).to_homogeneous()
                * Scale3::from((self.max - self.min) / 2.0).to_homogeneous(),
            ..Default::default()
        };
        let result = self.objects.iter().try_for_each(|object| {
            if cancellation.check().is_err() {
                return None;
            }
            let shape = VmShape::from(object.tree.clone()).try_into().ok()?;
            let octree = Octree::build(&shape, &settings)?;
            if cancellation.check().is_err() {
                return None;
            }
            let mesh = octree.walk_dual();
            if cancellation.check().is_err() {
                return None;
            }
            if !mesh
                .vertices
                .iter()
                .all(|v| v.iter().all(|x| x.is_finite()))
            {
                return None;
            }
            let offset = u32::try_from(out.vertices.len()).ok()?;
            u32::try_from(out.vertices.len().checked_add(mesh.vertices.len())?).ok()?;
            out.vertices
                .extend(mesh.vertices.into_iter().map(|position| Vertex {
                    position,
                    color: object.color.map(|n| f32::from(n) / 255.0),
                    normal: Normal::default(),
                }));
            out.indices.extend(
                mesh.triangles
                    .into_iter()
                    .flat_map(|t| [t.x, t.y, t.z].map(|i| offset + i as u32)),
            );
            Some(())
        });
        cancellation.check()?;
        Ok(result)
    }
}

fn view(preview: &VolumePreview, camera: Camera, pixels: PixelRenderSize) -> Option<View> {
    let center = (preview.min + preview.max) / 2.0;
    let radius = ((preview.max - preview.min) / 2.0).norm() * 1.05;
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let pitch = Rotation3::from_axis_angle(&Vector3::x_axis(), camera.pitch.to_radians());
    let yaw = Rotation3::from_axis_angle(&Vector3::z_axis(), camera.yaw.to_radians());
    // Match the implicit view's vertical framing, independent of pane width.
    let vertical_scale = camera.zoom / radius;
    Some(View {
        model_to_view: (yaw * pitch).inverse().to_homogeneous()
            * Translation3::from(-center).to_homogeneous(),
        projection: [
            vertical_scale * pixels.height() as f32 / pixels.width() as f32,
            vertical_scale,
            -0.5 / radius,
            0.5,
        ],
        width: pixels.width(),
        height: pixels.height(),
    })
}

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let (preview, depth) = read(input.value?)?;
    let geometry = generate(&preview, depth)?;
    Some(interactive_volume(
        drawing(&geometry, &preview, input.state, input.scale_factor)?,
        input,
    ))
}

pub(crate) fn read(value: &Value) -> Option<(VolumePreview, u8)> {
    let fields = value
        .as_record()?
        .get(&vocabulary::PREVIEW_MESH)?
        .as_record()?;
    Some((
        volume_preview_fields(fields)?,
        depth(fields.get(&vocabulary::MESH_DEPTH))?,
    ))
}

pub(crate) fn drawing(
    geometry: &Mesh,
    preview: &VolumePreview,
    state: Option<&Value>,
    scale_factor: f64,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    drawing_surface(geometry, None, preview, state, scale_factor)
}

pub(crate) fn drawing_surface(
    geometry: &Mesh,
    surface: Option<Surface>,
    preview: &VolumePreview,
    state: Option<&Value>,
    scale_factor: f64,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
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
        let scale = context.inputs.styles.scale;
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
