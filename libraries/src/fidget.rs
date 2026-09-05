//! Implicit fields as ordinary GID values, with Fidget as
//! one lowering and preview backend. Neither Grap nor GID knows about
//! the host representation.

use crate::{Library, absent, f32, name, presentation};
#[cfg(test)]
use fidget_engine::shape::EzShape;
#[cfg(not(target_arch = "wasm32"))]
use fidget_engine::wgpu::{Gpu, effects, voxel};
use fidget_engine::{
    context::Tree,
    raster::{
        pixel::{RenderConfig as PixelRenderConfig, RenderSize as PixelRenderSize},
        voxel::{GeometryPixel, RenderConfig as VoxelRenderConfig, RenderSize as VoxelRenderSize},
    },
    vm::VmShape,
};
use gid::{CellId, Cells, Value};

pub const ID: CellId = CellId::from_u128(0x5ccd78c1d555d14f55996f549d69f58a);
use grap_runtime::{Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use nalgebra::{Matrix4, Rotation3, Scale3, Translation3, Vector3};
use progred_display::{
    Layout, Paint, ProjectionInput, leaf, on_hover, on_state_drag, on_state_scroll,
};
use puri::{Affine, Command, Drawing, ImageAlphaType, ImageData, ImageFormat, Leaf};
use std::{cell::RefCell, rc::Rc};

const PREVIEW_SIZE: f64 = 256.0;
const ORBIT_DEGREES_PER_POINT: f32 = 180.0 / PREVIEW_SIZE as f32;

pub mod vocabulary {
    use gid::CellId;

    pub use crate::number::vocabulary::{LEFT, OPERAND, RIGHT};
    pub const FIDGET: CellId = CellId::from_u128(0x5653d5cc6cf43eb2291f9943c29eeab4);
    pub const AXIS: CellId = CellId::from_u128(0xfb2b3baa73025ae4b7b2aa97d65d1643);
    pub const X: CellId = CellId::from_u128(0x0192bad40c32c951e2237679084528bc);
    pub const Y: CellId = CellId::from_u128(0x213e54dd15ac9c9750308f35a606f56f);
    pub const Z: CellId = CellId::from_u128(0xc93e6bb743a9d90f85f8cdea3aabf5c1);
    pub const SUM: CellId = CellId::from_u128(0x208bf7b0ee1a002c66c86b34cf9eff3d);
    pub const SUBTRACT: CellId = CellId::from_u128(0x90e22494757f01a2ddf9bea409c186ae);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd2b341855e79890fc79bb562b109fcaf);
    pub const DIVIDE: CellId = CellId::from_u128(0xf9b35090433ceb932fb6da18d0dcc9bf);
    pub const MIN: CellId = CellId::from_u128(0x3fa78b77a43b285f26a97e7aaf5f7028);
    pub const MAX: CellId = CellId::from_u128(0x7082a82b3fd6dbc575aa5c85adfacc76);
    pub const NEGATE: CellId = CellId::from_u128(0x06eccebf47fc0faf7e33572d19c0d049);
    pub const ABS: CellId = CellId::from_u128(0x9f71b219018a191fab7b726732645ab1);
    pub const SQRT: CellId = CellId::from_u128(0xf275fe836c9d66fed1e0de4325e134c6);
    pub const SQUARE: CellId = CellId::from_u128(0x6164efe43a126d79bcd252a17a9453c1);
    pub const CIRCLE: CellId = CellId::from_u128(0xe467941b11832c441c98488dbdc5a540);
    pub const SPHERE: CellId = CellId::from_u128(0xc40de238820c3da73ec7780ddd438757);
    pub const TRANSLATE: CellId = CellId::from_u128(0xcc63ad1f4efa15b6f43f36b3cd11f3f6);
    pub const UNION: CellId = CellId::from_u128(0xb1d41a3c4f76c5929db19be204083d02);
    pub const INTERSECTION: CellId = CellId::from_u128(0x737a28cb9bd1a6d28604e8a68a909932);
    pub const DIFFERENCE: CellId = CellId::from_u128(0xc621348e3c35e46e87c1994eb1a50965);
    pub const PREVIEW: CellId = CellId::from_u128(0x69662683bafef0d10c88d46245cb638f);
    pub const PREVIEW_3D: CellId = CellId::from_u128(0x9a8142ecc3124e873dd2ad955d28fb85);
    pub const FIELD: CellId = CellId::from_u128(0x66bad5269b830181b840cf23a391b10e);
    pub const RADIUS: CellId = CellId::from_u128(0x64302843e07250efdb485267245c762e);
    pub const DELTA_X: CellId = CellId::from_u128(0x671936a24eb2d9d6e47ac5915a3e38c4);
    pub const DELTA_Y: CellId = CellId::from_u128(0x3dd683179b807ca200980eeffb90011f);
    pub const DELTA_Z: CellId = CellId::from_u128(0x48c4e6ddda046c53c02452d3d73e4388);
    pub const MIN_X: CellId = CellId::from_u128(0xf7906c57fc0b9f4b7013b19eef775c2d);
    pub const MAX_X: CellId = CellId::from_u128(0x9ffd33d780fb0665bcf2e4ec437079e8);
    pub const MIN_Y: CellId = CellId::from_u128(0xeabfd820f82b8e25c43cb6596cfcbd8f);
    pub const MAX_Y: CellId = CellId::from_u128(0xc9e7268eb34af7bea225df94841c1c27);
    pub const MIN_Z: CellId = CellId::from_u128(0x392a615eb91c6d64df871ec95831f89a);
    pub const MAX_Z: CellId = CellId::from_u128(0xda0ed6603210813bf52a20df7bf27c7d);
    pub const SLICE_Z: CellId = CellId::from_u128(0xa6712918cb0f80a44738a028c48b775b);
    pub const CAMERA: CellId = CellId::from_u128(0x78ba6e0120ecefc68469e7b67cf1f4af);
    pub const YAW: CellId = CellId::from_u128(0x6f226da2238a736c2f9ee38b40204f77);
    pub const PITCH: CellId = CellId::from_u128(0x9872a30927160707b693ffe1e6019fd9);
    pub const ZOOM: CellId = CellId::from_u128(0x745f4518cc847a4e7457e3426d010754);
    pub const INVALID_FIELD: CellId = CellId::from_u128(0xc26cfccc2a9fc752bf73c4359f2e9ade);
    pub const INVALID_BOUNDS: CellId = CellId::from_u128(0x64f01f9b22d96b4adfaadf21c2d6a74e);
    pub const INVALID_RADIUS: CellId = CellId::from_u128(0x36d2e0bdde4467b9b30e318578bbd79f);
}

fn node(marker: CellId, content: Value) -> Value {
    Value::record([(marker, content)])
}

fn unary(marker: CellId, operand: Value) -> Value {
    node(marker, Value::record([(vocabulary::OPERAND, operand)]))
}

fn binary(marker: CellId, left: Value, right: Value) -> Value {
    node(
        marker,
        Value::record([(vocabulary::LEFT, left), (vocabulary::RIGHT, right)]),
    )
}

fn evaluated(
    context: &mut grap_runtime::Context,
    call: Expression,
    environment: &Environment,
    field: CellId,
) -> Result<Option<Value>, Halt> {
    context
        .field(call, field)
        .map(|expression| context.eval(expression, environment))
        .transpose()
}

fn preview_value(
    context: &mut grap_runtime::Context,
    call: Expression,
    environment: &Environment,
    marker: CellId,
    bounds: impl IntoIterator<Item = (CellId, f32)>,
    valid: impl Fn(&[(CellId, Value, f32)]) -> bool,
) -> Result<Value, Halt> {
    let Some(field) = evaluated(context, call, environment, presentation::vocabulary::VALUE)?
    else {
        return Ok(context.missing_argument(presentation::vocabulary::VALUE));
    };
    if tree(&field).is_none() {
        return Ok(absent::with_reason(vocabulary::INVALID_FIELD));
    }
    let bounds = bounds
        .into_iter()
        .map(|(label, default)| {
            evaluated(context, call, environment, label).map(|value| {
                let value = value.unwrap_or_else(|| f32::value(default));
                f32::read(&value)
                    .filter(|number| number.is_finite())
                    .map(|number| (label, value, number))
            })
        })
        .collect::<Result<Option<Vec<_>>, _>>()?;
    Ok(match bounds.filter(|bounds| valid(bounds)) {
        Some(bounds) => node(
            marker,
            Value::record(
                [(vocabulary::FIELD, field)]
                    .into_iter()
                    .chain(bounds.into_iter().map(|(label, value, _)| (label, value))),
            ),
        ),
        None => absent::with_reason(vocabulary::INVALID_BOUNDS),
    })
}

fn unary_function(marker: CellId) -> ForeignFunction {
    ForeignFunction::new(move |context, call, environment| {
        let Some(operand) = evaluated(context, call, environment, vocabulary::OPERAND)? else {
            return Ok(context.missing_argument(vocabulary::OPERAND));
        };
        Ok(unary(marker, operand))
    })
}

fn binary_function(marker: CellId) -> ForeignFunction {
    ForeignFunction::new(move |context, call, environment| {
        let Some(left) = evaluated(context, call, environment, vocabulary::LEFT)? else {
            return Ok(context.missing_argument(vocabulary::LEFT));
        };
        let Some(right) = evaluated(context, call, environment, vocabulary::RIGHT)? else {
            return Ok(context.missing_argument(vocabulary::RIGHT));
        };
        Ok(binary(marker, left, right))
    })
}

fn circle_function(marker: CellId) -> ForeignFunction {
    ForeignFunction::new(move |context, call, environment| {
        let Some(radius) = evaluated(context, call, environment, vocabulary::RADIUS)? else {
            return Ok(context.missing_argument(vocabulary::RADIUS));
        };
        Ok(
            match f32::read(&radius).filter(|radius| radius.is_finite() && *radius >= 0.0) {
                Some(_) => node(marker, Value::record([(vocabulary::RADIUS, radius)])),
                None => absent::with_reason(vocabulary::INVALID_RADIUS),
            },
        )
    })
}

fn translate_function(
    context: &mut grap_runtime::Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    let Some(field) = evaluated(context, call, environment, vocabulary::FIELD)? else {
        return Ok(context.missing_argument(vocabulary::FIELD));
    };
    let mut values = Vec::with_capacity(4);
    values.push((vocabulary::FIELD, field));
    for coordinate in [
        vocabulary::DELTA_X,
        vocabulary::DELTA_Y,
        vocabulary::DELTA_Z,
    ] {
        let value =
            evaluated(context, call, environment, coordinate)?.unwrap_or_else(|| f32::value(0.0));
        if !matches!(f32::read(&value), Some(value) if value.is_finite()) {
            return Ok(absent::with_reason(vocabulary::INVALID_FIELD));
        }
        values.push((coordinate, value));
    }
    Ok(node(vocabulary::TRANSLATE, Value::record(values)))
}

fn preview_function(
    context: &mut grap_runtime::Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    preview_value(
        context,
        call,
        environment,
        vocabulary::PREVIEW,
        [
            (vocabulary::MIN_X, -100.0),
            (vocabulary::MAX_X, 100.0),
            (vocabulary::MIN_Y, -100.0),
            (vocabulary::MAX_Y, 100.0),
            (vocabulary::SLICE_Z, 0.0),
        ],
        |bounds| {
            matches!(
                bounds,
                [(_, _, min_x), (_, _, max_x), (_, _, min_y), (_, _, max_y), _]
                    if min_x < max_x && min_y < max_y
            )
        },
    )
}

fn preview_3d_function(
    context: &mut grap_runtime::Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    preview_value(
        context,
        call,
        environment,
        vocabulary::PREVIEW_3D,
        [
            (vocabulary::MIN_X, -80.0),
            (vocabulary::MAX_X, 80.0),
            (vocabulary::MIN_Y, -80.0),
            (vocabulary::MAX_Y, 80.0),
            (vocabulary::MIN_Z, -80.0),
            (vocabulary::MAX_Z, 80.0),
        ],
        |bounds| {
            matches!(
                bounds,
                [(_, _, min_x), (_, _, max_x), (_, _, min_y), (_, _, max_y), (_, _, min_z), (_, _, max_z)]
                    if min_x < max_x && min_y < max_y && min_z < max_z
            )
        },
    )
}

pub fn functions() -> ForeignFunctions {
    [
        (vocabulary::SUM, binary_function(vocabulary::SUM)),
        (vocabulary::SUBTRACT, binary_function(vocabulary::SUBTRACT)),
        (vocabulary::MULTIPLY, binary_function(vocabulary::MULTIPLY)),
        (vocabulary::DIVIDE, binary_function(vocabulary::DIVIDE)),
        (vocabulary::MIN, binary_function(vocabulary::MIN)),
        (vocabulary::MAX, binary_function(vocabulary::MAX)),
        (vocabulary::UNION, binary_function(vocabulary::UNION)),
        (
            vocabulary::INTERSECTION,
            binary_function(vocabulary::INTERSECTION),
        ),
        (
            vocabulary::DIFFERENCE,
            binary_function(vocabulary::DIFFERENCE),
        ),
    ]
    .into_iter()
    .chain([
        (vocabulary::NEGATE, unary_function(vocabulary::NEGATE)),
        (vocabulary::ABS, unary_function(vocabulary::ABS)),
        (vocabulary::SQRT, unary_function(vocabulary::SQRT)),
        (vocabulary::SQUARE, unary_function(vocabulary::SQUARE)),
    ])
    .fold(
        ForeignFunctions::default(),
        |functions, (cell, function)| functions.register(cell, function),
    )
    .register(vocabulary::CIRCLE, circle_function(vocabulary::CIRCLE))
    .register(vocabulary::SPHERE, circle_function(vocabulary::SPHERE))
    .register(
        vocabulary::TRANSLATE,
        ForeignFunction::new(translate_function),
    )
    .register(vocabulary::PREVIEW, ForeignFunction::new(preview_function))
    .register(
        vocabulary::PREVIEW_3D,
        ForeignFunction::new(preview_3d_function),
    )
}

fn one_marker(fields: &gid::Record) -> Option<CellId> {
    let markers = [
        vocabulary::AXIS,
        vocabulary::SUM,
        vocabulary::SUBTRACT,
        vocabulary::MULTIPLY,
        vocabulary::DIVIDE,
        vocabulary::MIN,
        vocabulary::MAX,
        vocabulary::NEGATE,
        vocabulary::ABS,
        vocabulary::SQRT,
        vocabulary::SQUARE,
        vocabulary::CIRCLE,
        vocabulary::SPHERE,
        vocabulary::TRANSLATE,
        vocabulary::UNION,
        vocabulary::INTERSECTION,
        vocabulary::DIFFERENCE,
    ];
    let mut present = markers
        .into_iter()
        .filter(|marker| fields.contains_key(marker));
    let marker = present.next()?;
    present.next().is_none().then_some(marker)
}

fn tree(value: &Value) -> Option<Tree> {
    if let Some(number) = f32::read(value) {
        return number.is_finite().then(|| Tree::constant(number.into()));
    }
    let fields = value.as_record()?;
    let marker = one_marker(fields)?;
    let content = fields.get(&marker)?;
    match marker {
        vocabulary::AXIS => match content.as_cell()? {
            vocabulary::X => Some(Tree::x()),
            vocabulary::Y => Some(Tree::y()),
            vocabulary::Z => Some(Tree::z()),
            _ => None,
        },
        vocabulary::CIRCLE | vocabulary::SPHERE => {
            let radius = f32::read(content.as_record()?.get(&vocabulary::RADIUS)?)?;
            let radial = if marker == vocabulary::CIRCLE {
                Tree::x().square() + Tree::y().square()
            } else {
                Tree::x().square() + Tree::y().square() + Tree::z().square()
            };
            Some(radial.sqrt() - radius)
        }
        vocabulary::TRANSLATE => {
            let fields = content.as_record()?;
            let field = tree(fields.get(&vocabulary::FIELD)?)?;
            let dx = f32::read(fields.get(&vocabulary::DELTA_X)?)?;
            let dy = f32::read(fields.get(&vocabulary::DELTA_Y)?)?;
            let dz = f32::read(fields.get(&vocabulary::DELTA_Z)?)?;
            Some(field.remap_xyz(Tree::x() - dx, Tree::y() - dy, Tree::z() - dz))
        }
        vocabulary::NEGATE | vocabulary::ABS | vocabulary::SQRT | vocabulary::SQUARE => {
            let operand = tree(content.as_record()?.get(&vocabulary::OPERAND)?)?;
            match marker {
                vocabulary::NEGATE => Some(-operand),
                vocabulary::ABS => Some(operand.abs()),
                vocabulary::SQRT => Some(operand.sqrt()),
                vocabulary::SQUARE => Some(operand.square()),
                _ => None,
            }
        }
        _ => {
            let fields = content.as_record()?;
            let left = tree(fields.get(&vocabulary::LEFT)?)?;
            let right = tree(fields.get(&vocabulary::RIGHT)?)?;
            match marker {
                vocabulary::SUM => Some(left + right),
                vocabulary::SUBTRACT => Some(left - right),
                vocabulary::MULTIPLY => Some(left * right),
                vocabulary::DIVIDE => Some(left / right),
                vocabulary::MIN | vocabulary::UNION => Some(left.min(right)),
                vocabulary::MAX | vocabulary::INTERSECTION => Some(left.max(right)),
                vocabulary::DIFFERENCE => Some(left.max(-right)),
                _ => None,
            }
        }
    }
}

struct SlicePreview {
    tree: Tree,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    z: f32,
}

fn slice_preview(value: &Value) -> Option<SlicePreview> {
    let fields = value.as_record()?.get(&vocabulary::PREVIEW)?.as_record()?;
    let preview = SlicePreview {
        tree: tree(fields.get(&vocabulary::FIELD)?)?,
        min_x: f32::read(fields.get(&vocabulary::MIN_X)?)?,
        max_x: f32::read(fields.get(&vocabulary::MAX_X)?)?,
        min_y: f32::read(fields.get(&vocabulary::MIN_Y)?)?,
        max_y: f32::read(fields.get(&vocabulary::MAX_Y)?)?,
        z: f32::read(fields.get(&vocabulary::SLICE_Z)?)?,
    };
    (preview.min_x < preview.max_x && preview.min_y < preview.max_y).then_some(preview)
}

fn slice_drawing(preview: SlicePreview, scale_factor: f64) -> Option<Drawing<Paint>> {
    let raster_size = (scale_factor.is_finite() && scale_factor > 0.0).then(|| {
        (PREVIEW_SIZE * scale_factor)
            .round()
            .clamp(1.0, f64::from(u32::MAX)) as u32
    })?;
    let half_width = (preview.max_x - preview.min_x) / 2.0;
    let half_height = (preview.max_y - preview.min_y) / 2.0;
    let tree = preview.tree.remap_xyz(
        Tree::x() * half_width + (preview.min_x + preview.max_x) / 2.0,
        Tree::y() * half_height + (preview.min_y + preview.max_y) / 2.0,
        Tree::constant(preview.z.into()),
    );
    let config = PixelRenderConfig::from_size(PixelRenderSize::from(raster_size));
    let image = config.run(VmShape::from(tree).try_into().ok()?)?;
    let rgba = image
        .iter()
        .flat_map(|pixel| {
            if pixel.inside() {
                [0, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            }
        })
        .collect::<Vec<u8>>();
    Some(Drawing {
        width: PREVIEW_SIZE,
        ascent: PREVIEW_SIZE / 2.0,
        descent: PREVIEW_SIZE / 2.0,
        commands: vec![Command::Image {
            image: ImageData {
                data: rgba.into(),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: raster_size,
                height: raster_size,
            },
            transform: Affine::scale(PREVIEW_SIZE / f64::from(raster_size)),
        }],
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Camera {
    yaw: f32,
    pitch: f32,
    zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 30.0,
            pitch: 60.0,
            zoom: 1.0,
        }
    }
}

fn camera(state: Option<&Value>) -> Camera {
    let fields = state
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&vocabulary::CAMERA))
        .and_then(Value::as_record);
    let read = |field, default| {
        fields
            .and_then(|fields| fields.get(&field))
            .and_then(f32::read)
            .filter(|value| value.is_finite())
            .unwrap_or(default)
    };
    Camera {
        yaw: read(vocabulary::YAW, Camera::default().yaw),
        pitch: read(vocabulary::PITCH, Camera::default().pitch).rem_euclid(360.0),
        zoom: read(vocabulary::ZOOM, Camera::default().zoom).clamp(0.05, 20.0),
    }
}

fn with_camera(state: Option<&Value>, camera: Camera) -> Value {
    let mut state = state
        .and_then(Value::as_record)
        .cloned()
        .unwrap_or_default();
    let mut fields = state
        .get(&vocabulary::CAMERA)
        .and_then(Value::as_record)
        .cloned()
        .unwrap_or_default();
    fields.insert(vocabulary::YAW, f32::value(camera.yaw));
    fields.insert(vocabulary::PITCH, f32::value(camera.pitch));
    fields.insert(vocabulary::ZOOM, f32::value(camera.zoom));
    state.insert(vocabulary::CAMERA, Value::Record(fields));
    Value::Record(state)
}

fn orbit_handler(state: Option<&Value>) -> progred_display::StateDragHandler {
    let state = state.cloned();
    let initial = camera(state.as_ref());
    Rc::new(move || {
        let state = state.clone();
        Box::new(move |event| {
            with_camera(
                state.as_ref(),
                Camera {
                    yaw: (initial.yaw - event.delta_x as f32 * ORBIT_DEGREES_PER_POINT)
                        .rem_euclid(360.0),
                    pitch: (initial.pitch - event.delta_y as f32 * ORBIT_DEGREES_PER_POINT)
                        .rem_euclid(360.0),
                    ..initial
                },
            )
        })
    })
}

fn zoom_handler(state: Option<&Value>) -> progred_display::StateScrollHandler {
    let state = state.cloned();
    let initial = camera(state.as_ref());
    Rc::new(move |event| {
        (event.delta_y != 0.0).then(|| {
            with_camera(
                state.as_ref(),
                Camera {
                    zoom: (initial.zoom * (event.delta_y as f32 * 0.0025).exp()).clamp(0.05, 20.0),
                    ..initial
                },
            )
        })
    })
}

struct VolumePreview {
    tree: Tree,
    min: Vector3<f32>,
    max: Vector3<f32>,
}

fn volume_preview(value: &Value) -> Option<VolumePreview> {
    let fields = value
        .as_record()?
        .get(&vocabulary::PREVIEW_3D)?
        .as_record()?;
    let preview = VolumePreview {
        tree: tree(fields.get(&vocabulary::FIELD)?)?,
        min: Vector3::new(
            f32::read(fields.get(&vocabulary::MIN_X)?)?,
            f32::read(fields.get(&vocabulary::MIN_Y)?)?,
            f32::read(fields.get(&vocabulary::MIN_Z)?)?,
        ),
        max: Vector3::new(
            f32::read(fields.get(&vocabulary::MAX_X)?)?,
            f32::read(fields.get(&vocabulary::MAX_Y)?)?,
            f32::read(fields.get(&vocabulary::MAX_Z)?)?,
        ),
    };
    preview
        .min
        .iter()
        .zip(preview.max.iter())
        .all(|(min, max)| min < max)
        .then_some(preview)
}

struct VolumeView {
    size: VoxelRenderSize,
    world_to_model: Matrix4<f32>,
}

fn volume_view(preview: &VolumePreview, camera: Camera, raster_size: u32) -> VolumeView {
    let center = (preview.min + preview.max) / 2.0;
    let half = (preview.max - preview.min) / 2.0;
    let pitch = Rotation3::from_axis_angle(&Vector3::x_axis(), camera.pitch.to_radians());
    let yaw = Rotation3::from_axis_angle(&Vector3::z_axis(), camera.yaw.to_radians());
    let rotation = yaw * pitch;
    let radius = half.norm() * 1.05;
    let depth = (f64::from(raster_size) * f64::from(camera.zoom.max(1.0)))
        .ceil()
        .min(f64::from(u32::MAX - 63)) as u32;
    let depth = depth.next_multiple_of(64);
    let depth_scale = depth as f32 / raster_size as f32;
    VolumeView {
        size: VoxelRenderSize::new(raster_size, raster_size, depth),
        world_to_model: Translation3::from(center).to_homogeneous()
            * rotation.to_homogeneous()
            * Scale3::new(
                radius / camera.zoom,
                radius / camera.zoom,
                radius / depth_scale,
            )
            .to_homogeneous(),
    }
}

fn cpu_volume(preview: &VolumePreview, camera: Camera, raster_size: u32) -> Option<Vec<u8>> {
    let view = volume_view(preview, camera, raster_size);
    let config = VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    };
    let shape = VmShape::from(preview.tree.clone()).try_into().ok()?;
    let image = config.run(shape)?;
    let light = Vector3::new(0.35, -0.45, 1.0).normalize();
    Some(
        image
            .iter()
            .flat_map(|pixel| shade_geometry(*pixel, light))
            .collect(),
    )
}

fn shade_geometry(pixel: GeometryPixel, light: Vector3<f32>) -> [u8; 4] {
    if pixel.depth == 0 {
        [0, 0, 0, 0]
    } else {
        let normal = Vector3::from(pixel.normal).normalize();
        let intensity = ((0.22 + 0.78 * normal.dot(&light).max(0.0)) * 255.0) as u8;
        [intensity, intensity, intensity, 255]
    }
}

struct PreviewRenderer {
    #[cfg(not(target_arch = "wasm32"))]
    gpu: GpuState,
}

impl Default for PreviewRenderer {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            gpu: GpuState::default(),
        }
    }
}

impl PreviewRenderer {
    fn render(
        &mut self,
        preview: &VolumePreview,
        camera: Camera,
        raster_size: u32,
    ) -> Option<Vec<u8>> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(image) = self.gpu.render(preview, camera, raster_size) {
            return Some(image);
        }
        cpu_volume(preview, camera, raster_size)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
enum GpuState {
    #[default]
    Uninitialized,
    Available(GpuRenderer),
    Unavailable,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuState {
    fn render(
        &mut self,
        preview: &VolumePreview,
        camera: Camera,
        raster_size: u32,
    ) -> Option<Vec<u8>> {
        if matches!(self, Self::Uninitialized) {
            let gpu = pollster::block_on(Gpu::init());
            *self = match gpu {
                Ok(gpu) => Self::Available(GpuRenderer::new(gpu)),
                Err(_) => Self::Unavailable,
            };
        }
        match self {
            Self::Available(renderer) => renderer.render(preview, camera, raster_size),
            Self::Uninitialized | Self::Unavailable => None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct GpuRenderer {
    gpu: Gpu,
    voxel: voxel::Context,
    effects: effects::Context,
    buffers: Option<GpuBuffers>,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuRenderer {
    fn new(gpu: Gpu) -> Self {
        Self {
            voxel: voxel::Context::new(&gpu),
            effects: effects::Context::new(&gpu),
            gpu,
            buffers: None,
        }
    }

    fn render(
        &mut self,
        preview: &VolumePreview,
        camera: Camera,
        raster_size: u32,
    ) -> Option<Vec<u8>> {
        let shape = self
            .voxel
            .shape(&VmShape::from(preview.tree.clone()))
            .ok()?;
        let Self {
            gpu,
            voxel,
            effects,
            buffers,
        } = self;
        let view = volume_view(preview, camera, raster_size);
        match buffers {
            Some(buffers) if buffers.size == view.size => {}
            Some(buffers)
                if buffers.size.width() == view.size.width()
                    && buffers.size.height() == view.size.height() =>
            {
                buffers.set_depth(voxel, effects, view.size)?;
            }
            _ => *buffers = Some(GpuBuffers::new(gpu, voxel, effects, view.size)?),
        }
        let GpuBuffers {
            voxel: voxel_buffers,
            merge,
            ssao,
            shade,
            read,
            ..
        } = buffers.as_mut()?;
        voxel
            .submit(
                &shape,
                voxel_buffers,
                None,
                &voxel::RenderConfig {
                    world_to_model: view.world_to_model,
                },
            )
            .ok()?;
        effects
            .submit_merge(&[voxel_buffers.image_storage_buffer()], true, merge)
            .ok()?;
        effects.submit_ssao(merge, ssao).ok()?;
        effects
            .submit_shade(merge, Some(ssao), shade, Some(read))
            .ok()?;
        Some(
            gpu.map(read)
                .image()
                .take()
                .0
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct GpuBuffers {
    size: VoxelRenderSize,
    voxel: voxel::Buffers,
    merge: effects::MergeBuffers,
    ssao: effects::SsaoBuffers,
    shade: effects::ShadeBuffers,
    read: fidget_engine::wgpu::buf::ImageReadBuffer<effects::ShadedImageTag>,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuBuffers {
    fn new(
        gpu: &Gpu,
        voxel: &voxel::Context,
        effects: &effects::Context,
        size: VoxelRenderSize,
    ) -> Option<Self> {
        let voxel = voxel.buffers(size).ok()?;
        let merge = effects.merge_buffers(size).ok()?;
        let ssao = effects.ssao_buffers(size).ok()?;
        let shade = effects
            .shade_buffers(PixelRenderSize::new(size.width(), size.height()))
            .ok()?;
        let read = gpu.read_buffer_for(shade.output());
        Some(Self {
            size,
            voxel,
            merge,
            ssao,
            shade,
            read,
        })
    }

    fn set_depth(
        &mut self,
        voxel: &voxel::Context,
        effects: &effects::Context,
        size: VoxelRenderSize,
    ) -> Option<()> {
        let merge = effects.merge_buffers(size).ok()?;
        voxel.set_buffers_image_size(&mut self.voxel, size).ok()?;
        self.size = size;
        self.merge = merge;
        Some(())
    }
}

fn volume_drawing(
    value: &Value,
    camera: Camera,
    scale_factor: f64,
    renderer: &mut PreviewRenderer,
) -> Option<Drawing<Paint>> {
    let preview = volume_preview(value);
    let raster_size = (scale_factor.is_finite() && scale_factor > 0.0).then(|| {
        (PREVIEW_SIZE * scale_factor)
            .round()
            .clamp(1.0, f64::from(u32::MAX)) as u32
    });
    preview.zip(raster_size).and_then(|(preview, raster_size)| {
        Some(Drawing {
            width: PREVIEW_SIZE,
            ascent: PREVIEW_SIZE / 2.0,
            descent: PREVIEW_SIZE / 2.0,
            commands: vec![Command::Image {
                image: ImageData {
                    data: renderer.render(&preview, camera, raster_size)?.into(),
                    format: ImageFormat::Rgba8,
                    alpha_type: ImageAlphaType::Alpha,
                    width: raster_size,
                    height: raster_size,
                },
                transform: Affine::scale(PREVIEW_SIZE / f64::from(raster_size)),
            }],
        })
    })
}

fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
    renderer: &RefCell<PreviewRenderer>,
) -> Option<Layout<World, Hover>> {
    if input
        .value
        .as_record()
        .is_some_and(|fields| fields.contains_key(&vocabulary::PREVIEW_3D))
    {
        let camera = camera(input.state);
        let drawing = leaf(Leaf::Drawing(volume_drawing(
            input.value,
            camera,
            input.scale_factor,
            &mut renderer.borrow_mut(),
        )?));
        let target = input.targets.current();
        let hover = target.hover;
        Some(on_state_scroll(
            on_state_drag(
                on_hover(drawing, hover.clone()),
                hover,
                target.select,
                orbit_handler(input.state),
            ),
            zoom_handler(input.state),
        ))
    } else {
        Some(leaf(Leaf::Drawing(slice_drawing(
            slice_preview(input.value)?,
            input.scale_factor,
        )?)))
    }
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::FIDGET, "fidget"),
        (vocabulary::AXIS, "axis"),
        (vocabulary::SUM, "+"),
        (vocabulary::SUBTRACT, "-"),
        (vocabulary::MULTIPLY, "*"),
        (vocabulary::DIVIDE, "/"),
        (vocabulary::MIN, "min"),
        (vocabulary::MAX, "max"),
        (vocabulary::NEGATE, "negate"),
        (vocabulary::ABS, "abs"),
        (vocabulary::SQRT, "sqrt"),
        (vocabulary::SQUARE, "square"),
        (vocabulary::CIRCLE, "circle"),
        (vocabulary::SPHERE, "sphere"),
        (vocabulary::TRANSLATE, "translate"),
        (vocabulary::UNION, "union"),
        (vocabulary::INTERSECTION, "intersection"),
        (vocabulary::DIFFERENCE, "difference"),
        (vocabulary::PREVIEW, "preview"),
        (vocabulary::PREVIEW_3D, "preview 3d"),
        (vocabulary::FIELD, "field"),
        (vocabulary::RADIUS, "radius"),
        (vocabulary::DELTA_X, "x"),
        (vocabulary::DELTA_Y, "y"),
        (vocabulary::DELTA_Z, "z"),
        (vocabulary::MIN_X, "minimum x"),
        (vocabulary::MAX_X, "maximum x"),
        (vocabulary::MIN_Y, "minimum y"),
        (vocabulary::MAX_Y, "maximum y"),
        (vocabulary::MIN_Z, "minimum z"),
        (vocabulary::MAX_Z, "maximum z"),
        (vocabulary::SLICE_Z, "slice z"),
        (vocabulary::CAMERA, "camera"),
        (vocabulary::YAW, "yaw"),
        (vocabulary::PITCH, "pitch"),
        (vocabulary::ZOOM, "zoom"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    for (cell, spelling) in [
        (vocabulary::INVALID_FIELD, "invalid field"),
        (vocabulary::INVALID_BOUNDS, "invalid preview bounds"),
        (vocabulary::INVALID_RADIUS, "invalid radius"),
    ] {
        cells.set_value(cell, absent::named_reason(spelling));
    }
    for (cell, spelling) in [
        (vocabulary::X, "x"),
        (vocabulary::Y, "y"),
        (vocabulary::Z, "z"),
    ] {
        cells.set_value(
            cell,
            name::record(spelling, [(vocabulary::AXIS, Value::from(cell))]),
        );
    }
    let renderer = Rc::new(RefCell::new(PreviewRenderer::default()));
    Library::named(
        "fidget",
        crate::Definitions::from_parts(cells, functions()),
        vec![progred_display::partial(move |input| {
            display(input, &renderer)
        })],
    )
    .with_root_completions([progred_display::Completion::generated("fidget", || {
        let cell = gid::new_cell_id();
        Value::record([
            (vocabulary::FIDGET, cell.into()),
            (
                crate::workspace::vocabulary::PANES,
                Value::record([(
                    crate::workspace::vocabulary::LEFT,
                    Value::list([Value::record([
                        (presentation::vocabulary::VALUE, cell.into()),
                        (
                            presentation::vocabulary::PROJECTION,
                            vocabulary::PREVIEW_3D.into(),
                        ),
                    ])]),
                )]),
            ),
        ])
    })
    .with_aliases(["sdf"])
    .with_detail("fidget library")
    .on_commit(crate::selection::pending_at(&[
        gid::Step::Key(vocabulary::FIDGET),
        gid::Step::Follow(gid::Resolution::Document),
    ]))])
    .with_root_field_completions([progred_display::Completion::new(
        "fidget",
        Value::from(vocabulary::FIDGET),
    )
    .with_aliases(["sdf"])
    .with_detail("fidget library")])
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_display::ProjectionTargets;
    use std::rc::Rc;

    fn call(function: CellId, arguments: impl IntoIterator<Item = (CellId, Value)>) -> Value {
        grap_runtime::call(Value::from(function), arguments)
    }

    fn evaluate(expression: &Value) -> Value {
        let library = library::<(), ()>();
        crate::test_evaluate(
            expression,
            |cell| library.value(cell).cloned(),
            &library.functions(),
            200,
        )
        .result
    }

    fn sample(tree: Tree, x: f32, y: f32, z: f32) -> f32 {
        let shape = VmShape::from(tree);
        let mut evaluator = VmShape::new_float_slice_eval();
        let tape = shape.ez_float_slice_tape();
        evaluator.eval(&tape, &[x], &[y], &[z]).unwrap()[0]
    }

    #[test]
    fn grap_constructs_a_field_that_fidget_evaluates() {
        let circle = call(vocabulary::CIRCLE, [(vocabulary::RADIUS, f32::value(10.0))]);
        let moved = call(
            vocabulary::TRANSLATE,
            [
                (vocabulary::FIELD, circle),
                (vocabulary::DELTA_X, f32::value(5.0)),
            ],
        );
        let field = evaluate(&moved);
        let tree = tree(&field).expect("GID field lowers to Fidget");

        assert!(sample(tree.clone(), 5.0, 0.0, 0.0) < 0.0);
        assert!(sample(tree, 20.0, 0.0, 0.0) > 0.0);
    }

    #[test]
    fn preview_is_an_ordinary_projected_value() {
        let expression = call(
            vocabulary::PREVIEW,
            [(
                presentation::vocabulary::VALUE,
                call(vocabulary::CIRCLE, [(vocabulary::RADIUS, f32::value(40.0))]),
            )],
        );
        let value = evaluate(&expression);
        let target = |_| progred_display::ProjectionTarget {
            select: Rc::new(|_: &mut ()| false),
            select_with: Rc::new(|_: &mut (), _| false),
            hover: (),
        };
        let renderer = RefCell::new(PreviewRenderer::default());
        let layout = display(
            &ProjectionInput {
                env: &NoEval,
                value: &value,
                scale_factor: 2.0,
                writable: false,
                selection: None,
                pending: None,
                state: None,
                targets: ProjectionTargets::new(&target),
            },
            &renderer,
        )
        .expect("preview projection");

        let Layout::Leaf(Leaf::Drawing(drawing)) = layout else {
            panic!("preview is one drawing leaf");
        };
        let [Command::Image { image, transform }] = drawing.commands.as_slice() else {
            panic!("preview drawing is one raster image");
        };
        assert_eq!((drawing.ascent, drawing.descent), (128.0, 128.0));
        assert_eq!((image.width, image.height), (512, 512));
        assert_eq!(*transform, Affine::scale(0.5));
        let alphas = image.data.as_ref().iter().skip(3).step_by(4);
        assert!(alphas.clone().any(|alpha| *alpha == 0));
        assert!(alphas.clone().any(|alpha| *alpha == 255));
    }

    #[test]
    fn grap_constructs_a_three_dimensional_preview() {
        let expression = call(
            vocabulary::PREVIEW_3D,
            [(
                presentation::vocabulary::VALUE,
                call(vocabulary::SPHERE, [(vocabulary::RADIUS, f32::value(40.0))]),
            )],
        );
        let value = evaluate(&expression);
        let preview = volume_preview(&value).expect("3D preview value");

        assert_eq!(preview.min, Vector3::new(-80.0, -80.0, -80.0));
        assert_eq!(preview.max, Vector3::new(80.0, 80.0, 80.0));
        assert!(sample(preview.tree.clone(), 0.0, 0.0, 0.0) < 0.0);
        assert!(sample(preview.tree, 80.0, 0.0, 0.0) > 0.0);
    }

    #[test]
    fn camera_gestures_update_open_projection_state() {
        let other_state = CellId::from_u128(1);
        let other_camera = CellId::from_u128(2);
        let state = Value::record([
            (other_state, Value::from(b"state".to_vec())),
            (
                vocabulary::CAMERA,
                Value::record([
                    (other_camera, Value::from(b"camera".to_vec())),
                    (vocabulary::YAW, f32::value(10.0)),
                ]),
            ),
        ]);
        let mut orbit = orbit_handler(Some(&state))();
        let state = orbit(progred_display::StateDragEvent {
            delta_x: 128.0,
            delta_y: -128.0,
        });

        assert_eq!(
            camera(Some(&state)),
            Camera {
                yaw: 280.0,
                pitch: 150.0,
                zoom: 1.0,
            }
        );
        let fields = state.as_record().expect("annotation record");
        assert!(fields.contains_key(&other_state));
        assert!(
            fields
                .get(&vocabulary::CAMERA)
                .and_then(Value::as_record)
                .is_some_and(|camera| camera.contains_key(&other_camera))
        );

        let state = zoom_handler(Some(&state))(progred_display::StateScrollEvent {
            delta_x: 0.0,
            delta_y: 100.0,
        })
        .expect("vertical scroll zooms");
        assert!((camera(Some(&state)).zoom - 0.25_f32.exp()).abs() < 0.0001);
    }

    #[test]
    fn zoom_keeps_volume_voxels_cubic_without_shortening_the_view() {
        let preview = VolumePreview {
            tree: Tree::from(0.0),
            min: Vector3::repeat(-1.0),
            max: Vector3::repeat(1.0),
        };
        let view = volume_view(
            &preview,
            Camera {
                yaw: 0.0,
                pitch: 0.0,
                zoom: 4.0,
            },
            256,
        );

        assert_eq!(view.size, VoxelRenderSize::new(256, 256, 1024));
        assert!((view.world_to_model[(0, 0)] - view.world_to_model[(2, 2)]).abs() < 0.0001);
        let screen_to_model = view.world_to_model * view.size.screen_to_world();
        assert!((screen_to_model[(0, 0)] - screen_to_model[(2, 2)]).abs() < 0.0001);
        assert!(
            (screen_to_model[(2, 3)].abs() - preview.max.z * 1.05_f32 * 3.0_f32.sqrt()).abs()
                < 0.0001
        );
    }

    #[test]
    fn horizontal_scroll_declines_camera_zoom() {
        assert!(
            zoom_handler(None)(progred_display::StateScrollEvent {
                delta_x: 10.0,
                delta_y: 0.0,
            })
            .is_none()
        );
    }

    struct NoEval;

    impl progred_display::Env for NoEval {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            unreachable!()
        }
    }
}
