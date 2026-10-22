//! Implicit fields as ordinary GID values, with Fidget as
//! one lowering and preview backend. Neither Grap nor GID knows about
//! the host representation.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, color, f32, name, presentation, representation};
#[cfg(not(target_arch = "wasm32"))]
use fidget_engine::wgpu::{
    Gpu, RenderShape,
    color::{ShapeColor, ShapeColorBuffers},
    voxel,
    voxel::effects,
};
use fidget_engine::{
    context::Tree,
    raster::{
        pixel::{RenderConfig as PixelRenderConfig, RenderSize as PixelRenderSize},
        voxel::{GeometryPixel, RenderConfig as VoxelRenderConfig, RenderSize as VoxelRenderSize},
    },
    vm::VmShape,
};
use gid::{CellId, Value};

mod completion;
pub(crate) mod interaction;
pub(crate) mod mesh;
mod projection;
pub(crate) mod raster;

pub const ID: CellId = CellId::from_u128(0x5ccd78c1d555d14f55996f549d69f58a);
use crate::display::widget::image::rgba_image;
use crate::display::{on_hover, on_state_drag, widget};
use ::grap::{Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use kurbo::{Affine, Size, Vec2};
use nalgebra::{Matrix4, Rotation3, Scale3, Translation3, Vector3};
use std::{cell::RefCell, rc::Rc};

#[cfg(not(target_arch = "wasm32"))]
fn gpu_result<T, E: std::fmt::Debug>(stage: &str, result: Result<T, E>) -> Option<T> {
    result
        .inspect_err(|error| eprintln!("Fidget GPU {stage} failed: {error:?}; using CPU fallback"))
        .ok()
}

const DEFAULT_PREVIEW_SIZE: f64 = 256.0;
const ORBIT_DEGREES_PER_POINT: f32 = 180.0 / 256.0;
/// Scrolling multiplies the zoom by `exp` of this per point.
const LOG_ZOOM_PER_POINT: f64 = 0.0025;
const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 20.0;

pub mod vocabulary {
    use gid::CellId;

    /// A node's operands: Fidget's own fields, whatever they share in
    /// name with Grap numbers'.
    pub const LEFT: CellId = CellId::from_u128(0x3cbf5357204947fdb4f26eadcbd777d6);
    pub const RIGHT: CellId = CellId::from_u128(0x6ab1e9c9a6604d82b1aabddcfa03d0d8);
    pub const OPERAND: CellId = CellId::from_u128(0xb66457fa59524110bb363bc28754cea0);
    pub const FIDGET: CellId = CellId::from_u128(0x5653d5cc6cf43eb2291f9943c29eeab4);
    pub const AXIS: CellId = CellId::from_u128(0xfb2b3baa73025ae4b7b2aa97d65d1643);
    pub const X: CellId = CellId::from_u128(0x0192bad40c32c951e2237679084528bc);
    pub const Y: CellId = CellId::from_u128(0x213e54dd15ac9c9750308f35a606f56f);
    pub const Z: CellId = CellId::from_u128(0xc93e6bb743a9d90f85f8cdea3aabf5c1);
    pub const PREVIEW: CellId = CellId::from_u128(0x69662683bafef0d10c88d46245cb638f);
    pub const PREVIEW_3D: CellId = CellId::from_u128(0x9a8142ecc3124e873dd2ad955d28fb85);
    pub const PREVIEW_MESH: CellId = CellId::from_u128(0x95eec28ff8cc6ba67153f9dfc4021139);
    pub const MESH_DEPTH: CellId = CellId::from_u128(0x47114a01984bb53e1538906f13333f84);
    pub const INVALID_MESH_DEPTH: CellId = CellId::from_u128(0xf1130a671f8af0a4444438a64450496c);
    pub const FIELD: CellId = CellId::from_u128(0x66bad5269b830181b840cf23a391b10e);
    pub const SCENE: CellId = CellId::from_u128(0xd6757ca4cb992116fe31ede39530709d);
    pub const COLOR: CellId = CellId::from_u128(0xd6b4c91765541de967c02878c42d15ea);
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
    pub const INVALID_SCENE: CellId = CellId::from_u128(0x8e43712a94cf767dc9c2bf848d8bbf15);
    pub const INVALID_COLOR: CellId = CellId::from_u128(0x91720fe73042065ac464952efa8dddd3);
    pub const INVALID_BOUNDS: CellId = CellId::from_u128(0x64f01f9b22d96b4adfaadf21c2d6a74e);
    pub const INVALID_SIZE: CellId = CellId::from_u128(0x220fe6002d6a973da9c2ea1ff7fec98e);

    /// The kinds of Fidget's nodes, each a cell of its own.
    pub mod kind {
        use gid::CellId;
        pub const AXIS: CellId = CellId::from_u128(0xf82b802a154f47ab973d4a522fd3d034);
        pub const SUM: CellId = CellId::from_u128(0x7300fd0088314a1b800a9cb15b015ba8);
        pub const SUBTRACT: CellId = CellId::from_u128(0x4535d940d2f04affa350291dd96f06bc);
        pub const MULTIPLY: CellId = CellId::from_u128(0x2b80236eda7245d6a63b0da98da25850);
        pub const DIVIDE: CellId = CellId::from_u128(0x9f458eda134e4adc98661b07b5169223);
        pub const MIN: CellId = CellId::from_u128(0x15260240eede4a64b39ebe15898f9b81);
        pub const MAX: CellId = CellId::from_u128(0x968e2f1f111a4849a8449dbeddbe8412);
        pub const UNION: CellId = CellId::from_u128(0x2c71c43b442847dfa9ce0b522bb7dc4a);
        pub const INTERSECTION: CellId = CellId::from_u128(0xcb8c27ebd8374ca7a69ded3c7cd92686);
        pub const DIFFERENCE: CellId = CellId::from_u128(0x10f86aecbad3474fa912858a56ebdf94);
        pub const NEGATE: CellId = CellId::from_u128(0xbb4210cb36874d16bce9948bcabd4191);
        pub const ABS: CellId = CellId::from_u128(0x119eddd0f1404b34a6e8653279b69feb);
        pub const SQRT: CellId = CellId::from_u128(0xdb6e742a39104ba89f2216ad9ab75e99);
        pub const SQUARE: CellId = CellId::from_u128(0xd4f866cfe9014192a360d5567df9cbef);
        pub const SIN: CellId = CellId::from_u128(0x3e471136af054326b47891d045361fbc);
        pub const COS: CellId = CellId::from_u128(0xfc89c17ecb5147158f928fe4e89da1e3);
        pub const TRANSLATE: CellId = CellId::from_u128(0x25dba7fa2ee74c2b986ba6e28aa3cbdf);
    }
}

/// The nodes of two operands: each kind and its spelling.
const BINARY: [(CellId, &str); 9] = [
    (vocabulary::kind::SUM, "+"),
    (vocabulary::kind::SUBTRACT, "-"),
    (vocabulary::kind::MULTIPLY, "*"),
    (vocabulary::kind::DIVIDE, "/"),
    (vocabulary::kind::MIN, "min"),
    (vocabulary::kind::MAX, "max"),
    (vocabulary::kind::UNION, "union"),
    (vocabulary::kind::INTERSECTION, "intersection"),
    (vocabulary::kind::DIFFERENCE, "difference"),
];

/// The nodes of one operand, as [`BINARY`] lists those of two.
const UNARY: [(CellId, &str); 6] = [
    (vocabulary::kind::NEGATE, "negate"),
    (vocabulary::kind::ABS, "abs"),
    (vocabulary::kind::SQRT, "sqrt"),
    (vocabulary::kind::SQUARE, "square"),
    (vocabulary::kind::SIN, "sin"),
    (vocabulary::kind::COS, "cos"),
];

/// The kinds of node a field is built from.
fn node_kinds() -> impl Iterator<Item = CellId> {
    BINARY
        .into_iter()
        .chain(UNARY)
        .map(|(kind, _)| kind)
        .chain([vocabulary::kind::AXIS, vocabulary::kind::TRANSLATE])
}

/// Arithmetic, which reads infix as a number's does: those of [`BINARY`]
/// spelled as a symbol.
fn arithmetic() -> impl Iterator<Item = (CellId, &'static str)> {
    BINARY
        .into_iter()
        .filter(|(_, spelling)| representation::Precedence::of_symbol(spelling).is_some())
}

/// The kind of node `fields` says it is, if it's one of Fidget's.
fn node_kind(fields: &gid::Record) -> Option<CellId> {
    let kind = ::grap::isa::of(fields)?;
    node_kinds()
        .any(|candidate| candidate == kind)
        .then_some(kind)
}

#[cfg(test)]
fn node(kind: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::isa::record(kind, fields)
}

#[cfg(test)]
fn axis(axis: CellId) -> Value {
    node(vocabulary::kind::AXIS, [(vocabulary::AXIS, axis.into())])
}

#[cfg(test)]
fn unary(kind: CellId, operand: Value) -> Value {
    node(kind, [(vocabulary::OPERAND, operand)])
}

#[cfg(test)]
fn binary(kind: CellId, left: Value, right: Value) -> Value {
    node(kind, [(vocabulary::LEFT, left), (vocabulary::RIGHT, right)])
}

fn evaluated(
    context: &mut ::grap::Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
) -> Result<Option<Value>, Halt> {
    context
        .field(call, field)
        .map(|expression| context.eval_to_value(expression, environment))
        .transpose()
}

fn preview_value(
    context: &mut ::grap::Context,
    call: &Expression,
    environment: &Environment,
    marker: CellId,
    bounds: impl IntoIterator<Item = (CellId, f32)>,
    valid: impl Fn(&[(CellId, Value, f32)]) -> bool,
) -> Result<Value, Halt> {
    let Some(field) = evaluated(context, call, environment, presentation::vocabulary::VALUE)?
    else {
        return Ok(context.missing_argument(call, presentation::vocabulary::VALUE));
    };
    let validation = if matches!(marker, vocabulary::PREVIEW_3D | vocabulary::PREVIEW_MESH) {
        scene_objects(&field).map(|_| ())
    } else {
        tree(&field).map(|_| ()).ok_or(vocabulary::INVALID_FIELD)
    };
    Ok(match validation {
        Err(reason) => ::grap::absent::value(reason),
        Ok(()) => match size_arguments(context, call, environment)? {
            None => ::grap::absent::value(vocabulary::INVALID_SIZE),
            Some(size) => match bound_arguments(context, call, environment, bounds)?
                .filter(|bounds| valid(bounds))
            {
                Some(bounds) => Value::record([(
                    marker,
                    Value::record(
                        [(vocabulary::FIELD, field)]
                            .into_iter()
                            .chain(size)
                            .chain(bounds.into_iter().map(|(label, value, _)| (label, value))),
                    ),
                )]),
                None => ::grap::absent::value(vocabulary::INVALID_BOUNDS),
            },
        },
    })
}

/// A preview's width and height arguments, each defaulting to
/// `DEFAULT_PREVIEW_SIZE`; `None` when either isn't a positive finite number.
fn size_arguments(
    context: &mut ::grap::Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Option<Vec<(CellId, Value)>>, Halt> {
    [
        crate::libraries::layout::vocabulary::WIDTH,
        crate::libraries::layout::vocabulary::HEIGHT,
    ]
    .into_iter()
    .map(|label| {
        evaluated(context, call, environment, label).map(|value| {
            let value = value.unwrap_or_else(|| crate::libraries::f64::value(DEFAULT_PREVIEW_SIZE));
            crate::libraries::f64::read(&value)
                .filter(|number| number.is_finite() && *number > 0.0)
                .map(|_| (label, value))
        })
    })
    .collect()
}

/// A preview's `bounds` arguments, each with its default, as stored and as
/// read; `None` when one isn't a finite f32.
fn bound_arguments(
    context: &mut ::grap::Context,
    call: &Expression,
    environment: &Environment,
    bounds: impl IntoIterator<Item = (CellId, f32)>,
) -> Result<Option<Vec<(CellId, Value, f32)>>, Halt> {
    bounds
        .into_iter()
        .map(|(label, default)| {
            evaluated(context, call, environment, label).map(|value| {
                let value = value.unwrap_or_else(|| f32::value(default));
                f32::read(&value)
                    .filter(|number| number.is_finite())
                    .map(|number| (label, value, number))
            })
        })
        .collect()
}

fn preview_function(
    context: &mut ::grap::Context,
    call: &Expression,
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

pub(crate) fn preview_3d_function(
    context: &mut ::grap::Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    volume_preview_function(context, call, environment, vocabulary::PREVIEW_3D)
}

fn volume_preview_function(
    context: &mut ::grap::Context,
    call: &Expression,
    environment: &Environment,
    marker: CellId,
) -> Result<Value, Halt> {
    preview_value(
        context,
        call,
        environment,
        marker,
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

/// Fidget's previews, which a pane's viewport or a document's evaluation
/// calls to draw a field.
fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::PREVIEW,
            ForeignFunction::from_value(preview_function)
                .parameters([presentation::vocabulary::VALUE])
                .tracked(),
        )
        .register(
            vocabulary::PREVIEW_3D,
            ForeignFunction::from_value(preview_3d_function)
                .parameters([presentation::vocabulary::VALUE])
                .tracked(),
        )
        .register(
            vocabulary::PREVIEW_MESH,
            ForeignFunction::from_value(mesh::preview)
                .parameters([presentation::vocabulary::VALUE])
                .tracked(),
        )
}

/// The fields a node of `kind` requires, when it's one of Fidget's
/// operations.
fn parameters(kind: CellId) -> Option<&'static [CellId]> {
    if BINARY.iter().any(|(candidate, _)| *candidate == kind) {
        Some(&[vocabulary::LEFT, vocabulary::RIGHT])
    } else if UNARY.iter().any(|(candidate, _)| *candidate == kind) {
        Some(&[vocabulary::OPERAND])
    } else {
        None
    }
}

fn tree(value: &Value) -> Option<Tree> {
    use vocabulary::kind;
    if let Some(number) = f32::read(value) {
        return number.is_finite().then(|| Tree::constant(number));
    }
    let fields = value.as_record()?;
    let operand = |field| tree(fields.get(&field)?);
    match node_kind(fields)? {
        kind::AXIS => match fields.get(&vocabulary::AXIS)?.as_cell()? {
            vocabulary::X => Some(Tree::x()),
            vocabulary::Y => Some(Tree::y()),
            vocabulary::Z => Some(Tree::z()),
            _ => None,
        },
        kind::TRANSLATE => {
            let field = operand(vocabulary::FIELD)?;
            // An offset left out is none.
            let delta = |field| match fields.get(&field) {
                Some(value) => f32::read(value).filter(|delta| delta.is_finite()),
                None => Some(0.0),
            };
            let [dx, dy, dz] = [
                delta(vocabulary::DELTA_X)?,
                delta(vocabulary::DELTA_Y)?,
                delta(vocabulary::DELTA_Z)?,
            ];
            Some(field.remap_xyz(Tree::x() - dx, Tree::y() - dy, Tree::z() - dz))
        }
        kind @ (kind::NEGATE | kind::ABS | kind::SQRT | kind::SQUARE | kind::SIN | kind::COS) => {
            let operand = operand(vocabulary::OPERAND)?;
            Some(match kind {
                kind::NEGATE => -operand,
                kind::ABS => operand.abs(),
                kind::SQRT => operand.sqrt(),
                kind::SQUARE => operand.square(),
                kind::SIN => operand.sin(),
                _ => operand.cos(),
            })
        }
        kind => {
            let left = operand(vocabulary::LEFT)?;
            let right = operand(vocabulary::RIGHT)?;
            match kind {
                kind::SUM => Some(left + right),
                kind::SUBTRACT => Some(left - right),
                kind::MULTIPLY => Some(left * right),
                kind::DIVIDE => Some(left / right),
                kind::MIN | kind::UNION => Some(left.min(right)),
                kind::MAX | kind::INTERSECTION => Some(left.max(right)),
                kind::DIFFERENCE => Some(left.max(-right)),
                _ => None,
            }
        }
    }
}

struct SlicePreview {
    tree: Tree,
    size: Size,
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
        size: preview_size(fields)?,
        min_x: f32::read(fields.get(&vocabulary::MIN_X)?)?,
        max_x: f32::read(fields.get(&vocabulary::MAX_X)?)?,
        min_y: f32::read(fields.get(&vocabulary::MIN_Y)?)?,
        max_y: f32::read(fields.get(&vocabulary::MAX_Y)?)?,
        z: f32::read(fields.get(&vocabulary::SLICE_Z)?)?,
    };
    (preview.min_x < preview.max_x && preview.min_y < preview.max_y).then_some(preview)
}

fn slice_display<W: Host>(preview: SlicePreview, scale_factor: f64) -> Option<Layout<W>> {
    let raster_size = raster_size(preview.size, scale_factor)?;
    let minimum = raster_size.width().min(raster_size.height()) as f32;
    let half_width = (preview.max_x - preview.min_x) / 2.0;
    let half_height = (preview.max_y - preview.min_y) / 2.0;
    let tree = preview.tree.remap_xyz(
        Tree::x() * (half_width * minimum / raster_size.width() as f32)
            + (preview.min_x + preview.max_x) / 2.0,
        Tree::y() * (half_height * minimum / raster_size.height() as f32)
            + (preview.min_y + preview.max_y) / 2.0,
        Tree::constant(preview.z),
    );
    let config = PixelRenderConfig::from_size(raster_size);
    let image = config.run(VmShape::from(tree).try_into().ok()?);
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
    Some(image_layout(preview.size, raster_size, rgba))
}

fn preview_size(fields: &gid::Record) -> Option<Size> {
    let dimension = |label| {
        crate::libraries::f64::read(fields.get(&label)?)
            .filter(|number| number.is_finite() && *number > 0.0)
    };
    Some(Size::new(
        dimension(crate::libraries::layout::vocabulary::WIDTH)?,
        dimension(crate::libraries::layout::vocabulary::HEIGHT)?,
    ))
}

fn raster_size(size: Size, scale: f64) -> Option<PixelRenderSize> {
    let dimension = |length| {
        let pixels: f64 = length * scale;
        (pixels.is_finite() && pixels > 0.0 && pixels <= f64::from(u32::MAX))
            .then(|| pixels.round().max(1.0) as u32)
    };
    Some(PixelRenderSize::new(
        dimension(size.width)?,
        dimension(size.height)?,
    ))
}

fn image_layout<W: Host>(size: Size, raster_size: PixelRenderSize, rgba: Vec<u8>) -> Layout<W> {
    widget::image::image(
        size,
        rgba_image(raster_size.width(), raster_size.height(), rgba),
    )
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
        zoom: read(vocabulary::ZOOM, Camera::default().zoom).clamp(MIN_ZOOM, MAX_ZOOM),
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

fn orbit_handler(state: Option<&Value>) -> crate::display::StateDragHandler {
    let state = state.cloned();
    let initial = camera(state.as_ref());
    Rc::new(move || {
        let state = state.clone();
        Box::new(move |event, _coalesced| {
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

fn scroll_zoom(
    state: Option<&Value>,
    delta: Vec2,
) -> (Option<Value>, puri::handler::ScrollOutcome<Vec2>) {
    let initial = camera(state);
    let requested = initial.zoom * (delta.y as f32 * LOG_ZOOM_PER_POINT as f32).exp();
    let zoom = requested.clamp(MIN_ZOOM, MAX_ZOOM);
    if zoom == initial.zoom {
        (None, puri::handler::ScrollOutcome::unhandled(delta))
    } else {
        (
            Some(with_camera(state, Camera { zoom, ..initial })),
            puri::handler::ScrollOutcome::handled_leaving(Vec2::new(
                delta.x,
                if zoom == requested {
                    0.0
                } else {
                    delta.y - (f64::from(zoom) / f64::from(initial.zoom)).ln() / LOG_ZOOM_PER_POINT
                },
            )),
        )
    }
}

fn pinch_zoom(state: Option<&Value>, delta: f32) -> Option<Value> {
    let initial = camera(state);
    let zoom = (initial.zoom * (1.0 + delta)).clamp(MIN_ZOOM, MAX_ZOOM);
    (delta.is_finite() && zoom != initial.zoom)
        .then(|| with_camera(state, Camera { zoom, ..initial }))
}

use volume::SceneObject;

fn scene_objects(value: &Value) -> Result<Vec<SceneObject>, CellId> {
    let Some(scene) = value
        .as_record()
        .and_then(|fields| fields.get(&vocabulary::SCENE))
    else {
        return Ok(vec![SceneObject {
            tree: tree(value).ok_or(vocabulary::INVALID_FIELD)?,
            color: [255; 3],
        }]);
    };
    let objects = scene.as_list().ok_or(vocabulary::INVALID_SCENE)?;
    // Fidget's merged voxel stores the object index in a u16.
    if objects.len() > usize::from(u16::MAX) + 1 {
        return Err(vocabulary::INVALID_SCENE);
    }
    objects
        .iter()
        .map(|(_, object)| {
            let fields = object.as_record().ok_or(vocabulary::INVALID_SCENE)?;
            let field = fields
                .get(&vocabulary::FIELD)
                .ok_or(vocabulary::INVALID_SCENE)?;
            let color = fields
                .get(&vocabulary::COLOR)
                .map(|value| color::read_opaque(value).ok_or(vocabulary::INVALID_COLOR))
                .transpose()?
                .unwrap_or([255; 3]);
            Ok(SceneObject {
                tree: tree(field).ok_or(vocabulary::INVALID_FIELD)?,
                color,
            })
        })
        .collect()
}

#[derive(Clone, PartialEq)]
pub(crate) struct VolumePreview {
    pub(crate) objects: Vec<SceneObject>,
    size: Size,
    min: Vector3<f32>,
    max: Vector3<f32>,
}

pub(crate) fn volume_preview(value: &Value) -> Option<VolumePreview> {
    volume_preview_fields(
        value
            .as_record()?
            .get(&vocabulary::PREVIEW_3D)?
            .as_record()?,
    )
}

fn volume_preview_fields(fields: &gid::Record) -> Option<VolumePreview> {
    let preview = VolumePreview {
        objects: scene_objects(fields.get(&vocabulary::FIELD)?).ok()?,
        size: preview_size(fields)?,
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
        .all(|(min, max)| min.is_finite() && max.is_finite() && min < max)
        .then_some(preview)
}

struct VolumeView {
    size: VoxelRenderSize,
    world_to_model: Matrix4<f32>,
}

fn volume_view(
    preview: &VolumePreview,
    camera: Camera,
    raster_size: PixelRenderSize,
) -> VolumeView {
    volume_view_bounds(preview.min, preview.max, camera, raster_size)
}

/// The center of the box from `min` to `max`, and the radius a view frames
/// around it: half its diagonal, with a margin. The implicit and mesh views
/// share it, so that a refined preview's two images agree.
fn framing(min: Vector3<f32>, max: Vector3<f32>) -> (Vector3<f32>, f32) {
    ((min + max) / 2.0, ((max - min) / 2.0).norm() * 1.05)
}

/// The camera's rotation from view to model: pitch about X, then yaw about Z.
fn orientation(camera: Camera) -> Rotation3<f32> {
    Rotation3::from_axis_angle(&Vector3::z_axis(), camera.yaw.to_radians())
        * Rotation3::from_axis_angle(&Vector3::x_axis(), camera.pitch.to_radians())
}

fn volume_view_bounds(
    min: Vector3<f32>,
    max: Vector3<f32>,
    camera: Camera,
    raster_size: PixelRenderSize,
) -> VolumeView {
    let (center, radius) = framing(min, max);
    let minimum = raster_size.width().min(raster_size.height());
    let height = raster_size.height();
    let depth = (f64::from(height) * f64::from(camera.zoom.max(1.0)))
        .ceil()
        .min(f64::from(u32::MAX - 63)) as u32;
    let depth = depth.next_multiple_of(64);
    let depth_scale = depth as f32 / minimum as f32;
    // Frame vertically so a pane divider changes the horizontal field of view,
    // not the model's scale. Fidget normalizes by the shortest axis instead.
    let image_scale = minimum as f32 / height as f32;
    VolumeView {
        size: VoxelRenderSize::new(raster_size.width(), raster_size.height(), depth),
        world_to_model: Translation3::from(center).to_homogeneous()
            * orientation(camera).to_homogeneous()
            * Scale3::new(
                radius / camera.zoom * image_scale,
                radius / camera.zoom * image_scale,
                radius / depth_scale,
            )
            .to_homogeneous()
            // Fidget samples integer XY coordinates; match rasterizer pixel
            // centers (its screen transform also flips Y around height - 1).
            * Translation3::new(1.0 / minimum as f32, 1.0 / minimum as f32, 0.0)
                .to_homogeneous(),
    }
}

fn cpu_volume(
    objects: &[SceneObject],
    view: &VolumeView,
    cancellation: &incremental::Cancellation,
) -> Option<Vec<u8>> {
    raster::SoftwareScene::new(objects, cancellation)
        .ok()
        .flatten()?
        .render(view, cancellation)
}

#[derive(Default)]
struct PreviewRenderer {
    /// None before the first render, and while the process has no GPU.
    #[cfg(not(target_arch = "wasm32"))]
    gpu: Option<GpuRenderer>,
}

impl PreviewRenderer {
    fn render(
        &mut self,
        preview: &VolumePreview,
        camera: Camera,
        raster_size: PixelRenderSize,
    ) -> Option<Vec<u8>> {
        let view = volume_view(preview, camera, raster_size);
        if preview.objects.is_empty() {
            return Some(vec![
                0;
                raster_size.width() as usize
                    * raster_size.height() as usize
                    * 4
            ]);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(image) = gpu_render(&mut self.gpu, &preview.objects, &view) {
            return Some(image);
        }
        cpu_volume(&preview.objects, &view, &Default::default())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn gpu_render(
    gpu: &mut Option<GpuRenderer>,
    objects: &[SceneObject],
    view: &VolumeView,
) -> Option<Vec<u8>> {
    if gpu.is_none() {
        *gpu = shared_gpu().map(GpuRenderer::new);
    }
    gpu.as_mut()?.render(objects, view)
}

/// The process's one GPU device, or `None` after a failed attempt, which isn't
/// retried. Initializing once matters for correctness, not just speed: the
/// Vulkan loader crashes when threads probe drivers concurrently, as parallel
/// tests did in the build sandbox, where NVIDIA's driver fails to initialize.
#[cfg(not(target_arch = "wasm32"))]
fn shared_gpu() -> Option<Gpu> {
    static GPU: std::sync::OnceLock<Option<Gpu>> = std::sync::OnceLock::new();
    GPU.get_or_init(|| match pollster::block_on(Gpu::init()) {
        Ok(gpu) => Some(gpu),
        Err(error) => {
            eprintln!("Fidget GPU initialization failed: {error:?}; using CPU fallback");
            None
        }
    })
    .clone()
}

#[cfg(not(target_arch = "wasm32"))]
struct GpuRenderer {
    gpu: Gpu,
    voxel: voxel::Context,
    effects: effects::Context,
    voxel_workspace: voxel::Workspace,
    merge: effects::MergeWorkspace,
    color_workspace: effects::ColorWorkspace,
    ssao: effects::SsaoWorkspace,
    shade: effects::ShadeWorkspace,
    read: fidget_engine::wgpu::buf::ReadBuffer<effects::ShadedImageTag>,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuRenderer {
    fn new(gpu: Gpu) -> Self {
        let voxel = voxel::Context::new(&gpu);
        let effects = effects::Context::new(&gpu);
        let shade = effects.shade_workspace();
        Self {
            voxel_workspace: voxel.workspace(),
            merge: effects.merge_workspace(),
            color_workspace: effects.color_workspace(),
            ssao: effects.ssao_workspace(),
            read: gpu.read_buffer_for(shade.output()),
            shade,
            voxel,
            effects,
            gpu,
        }
    }

    fn render(&mut self, objects: &[SceneObject], view: &VolumeView) -> Option<Vec<u8>> {
        let Self {
            gpu,
            voxel,
            effects,
            voxel_workspace,
            merge,
            color_workspace,
            ssao,
            shade,
            read,
        } = self;
        merge.reset();
        for object in objects {
            let shape = gpu_result(
                "bytecode",
                RenderShape::new(&VmShape::from(object.tree.clone())),
            )?;
            gpu_result(
                "voxel submission",
                voxel.submit(
                    &shape,
                    voxel_workspace,
                    &VoxelRenderConfig {
                        image_size: view.size,
                        world_to_model: view.world_to_model,
                    },
                ),
            )?;
            gpu_result(
                "merge submission",
                effects.submit_merge(
                    voxel_workspace.output(),
                    effects::MergeSettings::default(),
                    merge,
                ),
            )?;
        }
        let colors = objects
            .iter()
            .map(|object| {
                let [r, g, b] = object
                    .color
                    .map(|channel| VmShape::from(Tree::constant(f32::from(channel) / 255.0)));
                ShapeColor::Rgb { r, g, b }
            })
            .collect::<Vec<_>>();
        let colors = gpu_result("color bytecode", ShapeColorBuffers::new(&colors))?;
        gpu_result(
            "color submission",
            effects.submit_color(merge, &view.world_to_model, &colors, color_workspace, shade),
        )?;
        gpu_result("SSAO submission", effects.submit_ssao(merge, ssao))?;
        gpu_result(
            "shade submission",
            effects.submit_shade(merge, Some(ssao), shade),
        )?;
        gpu.copy(shade.output(), read);
        Some(
            gpu.map(read)
                .to_vec()
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        )
    }
}

fn volume_display<W: Host>(
    preview: &VolumePreview,
    state: Option<&Value>,
    scale_factor: f64,
    renderer: &mut PreviewRenderer,
) -> Option<Layout<W>> {
    let raster_size = raster_size(preview.size, scale_factor)?;
    Some(image_layout(
        preview.size,
        raster_size,
        renderer.render(preview, camera(state), raster_size)?,
    ))
}

fn display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    renderer: &RefCell<PreviewRenderer>,
) -> Option<Layout<W>> {
    let value = input.value?;
    if value.contains_field(vocabulary::PREVIEW_3D) {
        let drawing = volume_display(
            &volume_preview(value.as_value())?,
            input.state,
            input.scale_factor,
            &mut renderer.borrow_mut(),
        )?;
        Some(interactive_volume(drawing, input))
    } else {
        value.field(vocabulary::PREVIEW)?;
        slice_display(slice_preview(value.as_value())?, input.scale_factor)
    }
}

pub(crate) fn interactive_volume<W: Host, V>(
    drawing: Layout<W>,
    input: &ProjectionInput<'_, W, V>,
) -> Layout<W> {
    let site = input.site();
    let hover = W::Hover::from(site.hover());
    crate::display::widget::before(
        on_state_drag(
            on_hover(drawing, hover.clone()),
            hover,
            crate::display::widget::navigation::selecting(site),
            orbit_handler(input.state),
        ),
        Rc::new(move |context| {
            let site = context.site();
            let activity = interaction::Interaction::at(context.viewing().computations(), &site);
            let pinch_activity = activity.clone();
            let pinch_site = site.clone();
            let scroll = crate::display::widget::scroll::scroll(
                context.viewing().scale(),
                move |world: &mut W, delta| {
                    let mut session = world.session();
                    let mut edit = site.open(&mut session);
                    let (state, outcome) = scroll_zoom(edit.state(), delta);
                    if let Some(state) = state {
                        edit.set_state(state);
                        activity.zoomed(session.timers, web_time::Instant::now());
                    }
                    outcome
                },
            );
            Box::new(move |output, placement| {
                scroll(output, placement);
                puri::interact::on_pinch(output, placement, move |world: &mut W, delta| {
                    let mut session = world.session();
                    let mut edit = pinch_site.open(&mut session);
                    if let Some(state) = pinch_zoom(edit.state(), delta) {
                        edit.set_state(state);
                        pinch_activity.zoomed(session.timers, web_time::Instant::now());
                    }
                    // Own the gesture even at a zoom limit.
                    true
                });
            })
        }),
    )
}

pub fn library<W: Host>() -> Library<W> {
    let renderer = Rc::new(RefCell::new(PreviewRenderer::default()));
    // Each of Fidget's nodes draws as its kind provides.
    let nodes = node_kinds()
        .map(|kind| (kind, crate::display::runtime_partial(projection::field)))
        .collect::<Vec<_>>();
    name::library_named(
        ID,
        "fidget",
        [
            (vocabulary::FIDGET, "fidget"),
            (vocabulary::AXIS, "axis"),
            (vocabulary::kind::AXIS, "axis"),
            (vocabulary::LEFT, "left"),
            (vocabulary::RIGHT, "right"),
            (vocabulary::OPERAND, "operand"),
            (vocabulary::kind::TRANSLATE, "translate"),
            (vocabulary::PREVIEW, "preview"),
            (vocabulary::PREVIEW_3D, "preview 3d"),
            (vocabulary::PREVIEW_MESH, "preview mesh"),
            (vocabulary::MESH_DEPTH, "mesh depth"),
            (vocabulary::FIELD, "field"),
            (vocabulary::SCENE, "scene"),
            (vocabulary::COLOR, "color"),
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
        ]
        .into_iter()
        .chain(
            UNARY
                .into_iter()
                .chain(BINARY)
                .filter(|(_, spelling)| representation::Precedence::of_symbol(spelling).is_none()),
        )
        .map(|(cell, spelling)| (cell, crate::libraries::text::value(spelling)))
        // An arithmetic symbol carries Fidget's tag, telling it from a
        // number's.
        .chain(arithmetic().map(|(kind, spelling)| {
            (
                kind,
                name::formatted([
                    crate::libraries::text::value(spelling),
                    name::subscript(vocabulary::FIDGET.into()),
                ]),
            )
        }))
        .chain(
            [
                (vocabulary::INVALID_FIELD, "invalid field"),
                (vocabulary::INVALID_SCENE, "invalid scene"),
                (vocabulary::INVALID_COLOR, "invalid opaque color"),
                (vocabulary::INVALID_BOUNDS, "invalid preview bounds"),
                (vocabulary::INVALID_SIZE, "invalid preview size"),
                (
                    vocabulary::INVALID_MESH_DEPTH,
                    "mesh depth must be from 1 to 8",
                ),
            ]
            .map(|(cell, spelling)| (cell, crate::libraries::text::value(spelling))),
        ),
        functions(),
    )
    .with_values(
        [
            (vocabulary::X, "x"),
            (vocabulary::Y, "y"),
            (vocabulary::Z, "z"),
        ]
        .map(|(cell, spelling)| {
            (
                cell,
                name::record(
                    spelling,
                    [
                        (::grap::isa::ISA, vocabulary::kind::AXIS.into()),
                        (vocabulary::AXIS, Value::from(cell)),
                    ],
                ),
            )
        }),
    )
    .with_projections_for(nodes)
    .with_projection(crate::display::compose_partials([
        crate::display::runtime_partial(move |input| display(input, &renderer)),
        crate::display::runtime_partial(mesh::display),
    ]))
    .with_completions(completion::offers)
}

#[cfg(test)]
mod tests;
