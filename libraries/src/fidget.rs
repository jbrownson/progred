//! Implicit fields as ordinary GID values, with Fidget as
//! one lowering and preview backend. Neither Grap nor GID knows about
//! the host representation.

use crate::{Library, absent, f32, name};
#[cfg(test)]
use fidget_engine::shape::EzShape;
use fidget_engine::{
    context::Tree,
    raster::pixel::{RenderConfig, RenderSize},
    vm::VmShape,
};
use gid::{CellId, Cells, Value};
use grap_runtime::{Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{Face, Layout, Paint, ProjectionInput, leaf};
use puri::{
    Affine, Command, Drawing, ImageAlphaType, ImageData, ImageFormat, Leaf, Rect, Shape, Stroke,
};

const PREVIEW_SIZE: f64 = 256.0;

pub mod vocabulary {
    use gid::CellId;

    pub const AXIS: CellId = CellId::from_u128(0xfb2b3baa73025ae4b7b2aa97d65d1643);
    pub const X: CellId = CellId::from_u128(0x0192bad40c32c951e2237679084528bc);
    pub const Y: CellId = CellId::from_u128(0x213e54dd15ac9c9750308f35a606f56f);
    pub const Z: CellId = CellId::from_u128(0xc93e6bb743a9d90f85f8cdea3aabf5c1);
    pub const ADD: CellId = CellId::from_u128(0x208bf7b0ee1a002c66c86b34cf9eff3d);
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
    pub const FIELD: CellId = CellId::from_u128(0x66bad5269b830181b840cf23a391b10e);
    pub const LEFT: CellId = CellId::from_u128(0x74bfc2a4db82ecc1f6af915d46e8f68d);
    pub const RIGHT: CellId = CellId::from_u128(0x8dfdc3487bdfbc209ffb77c74433ef04);
    pub const OPERAND: CellId = CellId::from_u128(0x5b4a1c80f5ad91bbfe650e6e81afb1ea);
    pub const RADIUS: CellId = CellId::from_u128(0x64302843e07250efdb485267245c762e);
    pub const DELTA_X: CellId = CellId::from_u128(0x671936a24eb2d9d6e47ac5915a3e38c4);
    pub const DELTA_Y: CellId = CellId::from_u128(0x3dd683179b807ca200980eeffb90011f);
    pub const DELTA_Z: CellId = CellId::from_u128(0x48c4e6ddda046c53c02452d3d73e4388);
    pub const MIN_X: CellId = CellId::from_u128(0xf7906c57fc0b9f4b7013b19eef775c2d);
    pub const MAX_X: CellId = CellId::from_u128(0x9ffd33d780fb0665bcf2e4ec437079e8);
    pub const MIN_Y: CellId = CellId::from_u128(0xeabfd820f82b8e25c43cb6596cfcbd8f);
    pub const MAX_Y: CellId = CellId::from_u128(0xc9e7268eb34af7bea225df94841c1c27);
    pub const SLICE_Z: CellId = CellId::from_u128(0xa6712918cb0f80a44738a028c48b775b);
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
    let Some(field) = evaluated(context, call, environment, vocabulary::FIELD)? else {
        return Ok(context.missing_argument(vocabulary::FIELD));
    };
    if tree(&field).is_none() {
        return Ok(absent::with_reason(vocabulary::INVALID_FIELD));
    }
    let bounds = [
        (vocabulary::MIN_X, -100.0),
        (vocabulary::MAX_X, 100.0),
        (vocabulary::MIN_Y, -100.0),
        (vocabulary::MAX_Y, 100.0),
        (vocabulary::SLICE_Z, 0.0),
    ]
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
    Ok(match bounds {
        Some(bounds)
            if matches!(
                bounds.as_slice(),
                [(_, _, min_x), (_, _, max_x), (_, _, min_y), (_, _, max_y), _]
                    if min_x < max_x && min_y < max_y
            ) =>
        {
            node(
                vocabulary::PREVIEW,
                Value::record(
                    [(vocabulary::FIELD, field)]
                        .into_iter()
                        .chain(bounds.into_iter().map(|(label, value, _)| (label, value))),
                ),
            )
        }
        _ => absent::with_reason(vocabulary::INVALID_BOUNDS),
    })
}

pub fn functions() -> ForeignFunctions {
    [
        (vocabulary::ADD, binary_function(vocabulary::ADD)),
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
}

fn one_marker(fields: &gid::Record) -> Option<CellId> {
    let markers = [
        vocabulary::AXIS,
        vocabulary::ADD,
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
                vocabulary::ADD => Some(left + right),
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

struct Preview {
    tree: Tree,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    z: f32,
}

fn preview(value: &Value) -> Option<Preview> {
    let fields = value.as_record()?.get(&vocabulary::PREVIEW)?.as_record()?;
    let preview = Preview {
        tree: tree(fields.get(&vocabulary::FIELD)?)?,
        min_x: f32::read(fields.get(&vocabulary::MIN_X)?)?,
        max_x: f32::read(fields.get(&vocabulary::MAX_X)?)?,
        min_y: f32::read(fields.get(&vocabulary::MIN_Y)?)?,
        max_y: f32::read(fields.get(&vocabulary::MAX_Y)?)?,
        z: f32::read(fields.get(&vocabulary::SLICE_Z)?)?,
    };
    (preview.min_x < preview.max_x && preview.min_y < preview.max_y).then_some(preview)
}

fn drawing(preview: Preview, scale_factor: f64) -> Option<Drawing<Paint>> {
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
    let config = RenderConfig::from_size(RenderSize::from(raster_size));
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
        commands: vec![
            Command::Image {
                image: ImageData {
                    data: rgba.into(),
                    format: ImageFormat::Rgba8,
                    alpha_type: ImageAlphaType::Alpha,
                    width: raster_size,
                    height: raster_size,
                },
                transform: Affine::scale(PREVIEW_SIZE / f64::from(raster_size)),
            },
            Command::Stroke {
                shape: Shape::Rect(Rect::new(0.5, 0.5, PREVIEW_SIZE - 0.5, PREVIEW_SIZE - 0.5)),
                style: Stroke::new(1.0),
                paint: Paint::Face(Face::Dim),
                transform: Affine::IDENTITY,
            },
        ],
    })
}

pub fn display<World, Hover>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    Some(leaf(Leaf::Drawing(drawing(
        preview(input.value)?,
        input.scale_factor,
    )?)))
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::AXIS, "axis"),
        (vocabulary::ADD, "fidget +"),
        (vocabulary::SUBTRACT, "fidget -"),
        (vocabulary::MULTIPLY, "fidget *"),
        (vocabulary::DIVIDE, "fidget /"),
        (vocabulary::MIN, "fidget min"),
        (vocabulary::MAX, "fidget max"),
        (vocabulary::NEGATE, "fidget negate"),
        (vocabulary::ABS, "fidget abs"),
        (vocabulary::SQRT, "fidget sqrt"),
        (vocabulary::SQUARE, "fidget square"),
        (vocabulary::CIRCLE, "fidget circle"),
        (vocabulary::SPHERE, "fidget sphere"),
        (vocabulary::TRANSLATE, "fidget translate"),
        (vocabulary::UNION, "fidget union"),
        (vocabulary::INTERSECTION, "fidget intersection"),
        (vocabulary::DIFFERENCE, "fidget difference"),
        (vocabulary::PREVIEW, "fidget preview"),
        (vocabulary::FIELD, "field"),
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
        (vocabulary::OPERAND, "operand"),
        (vocabulary::RADIUS, "radius"),
        (vocabulary::DELTA_X, "x"),
        (vocabulary::DELTA_Y, "y"),
        (vocabulary::DELTA_Z, "z"),
        (vocabulary::MIN_X, "minimum x"),
        (vocabulary::MAX_X, "maximum x"),
        (vocabulary::MIN_Y, "minimum y"),
        (vocabulary::MAX_Y, "maximum y"),
        (vocabulary::SLICE_Z, "slice z"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    for (cell, spelling) in [
        (vocabulary::INVALID_FIELD, "invalid fidget field"),
        (vocabulary::INVALID_BOUNDS, "invalid fidget preview bounds"),
        (vocabulary::INVALID_RADIUS, "invalid fidget radius"),
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
    Library {
        cells,
        functions: functions(),
        projections: vec![display::<World, Hover>],
    }
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
        grap_runtime::evaluate(
            expression,
            |cell| library.cells.value(cell).cloned(),
            &library.functions,
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
                vocabulary::FIELD,
                call(vocabulary::CIRCLE, [(vocabulary::RADIUS, f32::value(40.0))]),
            )],
        );
        let value = evaluate(&expression);
        let target = |_| progred_display::ProjectionTarget {
            select: Rc::new(|_: &mut ()| false),
            select_with: Rc::new(|_: &mut (), _| false),
            hover: (),
        };
        let layout = display(&ProjectionInput {
            env: &NoEval,
            value: &value,
            scale_factor: 2.0,
            writable: false,
            selection: None,
            state: None,
            targets: ProjectionTargets::new(&target),
        })
        .expect("preview projection");

        let Layout::Leaf(Leaf::Drawing(drawing)) = layout else {
            panic!("preview is one drawing leaf");
        };
        let [Command::Image { image, transform }, Command::Stroke { .. }] =
            drawing.commands.as_slice()
        else {
            panic!("preview is one raster image inside one border");
        };
        assert_eq!((drawing.ascent, drawing.descent), (128.0, 128.0));
        assert_eq!((image.width, image.height), (512, 512));
        assert_eq!(*transform, Affine::scale(0.5));
        let alphas = image.data.as_ref().iter().skip(3).step_by(4);
        assert!(alphas.clone().any(|alpha| *alpha == 0));
        assert!(alphas.clone().any(|alpha| *alpha == 255));
    }

    struct NoEval;

    impl progred_display::Env for NoEval {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            unreachable!()
        }
    }
}
