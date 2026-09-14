//! A path sink that builds Fidget geometry, sharing the ordinary 3D viewport.

use super::{Error, argument, invalid, number, paths::*, result, run, vocabulary::*};
use crate::display::{Layout, ProjectionInput};
use crate::libraries::{absent, color, f64, fidget, layout, presentation};
use ::grap::{Context, Environment, Expression, Halt};
use fidget_engine::context::Tree;
use gid::Value;
use std::{cell::RefCell, rc::Rc};

pub(super) struct Tubes {
    radius: f32,
    previous: Option<[f32; 3]>,
    path: Option<Tree>,
    paths: Vec<Tree>,
}

impl Tubes {
    pub(super) fn new(radius: f64) -> Option<Self> {
        Some(Self {
            radius: read_radius(radius)?,
            previous: None,
            path: None,
            paths: Vec::new(),
        })
    }

    pub(super) fn finish(mut self) -> Vec<Tree> {
        self.paths.extend(self.path);
        self.paths
    }
}

pub(super) fn read_radius(radius: f64) -> Option<f32> {
    let radius = radius as f32;
    (radius > 0.0 && (radius * radius).is_finite() && radius * radius > 0.0).then_some(radius)
}

pub(super) fn coordinate(point: Point3) -> Result<[f32; 3], InvalidPath> {
    if !point.into_iter().all(f64::is_finite) {
        return Err(InvalidPath::NonFinitePoint);
    }
    let point = point.map(|n| n as f32);
    point
        .into_iter()
        .all(f32::is_finite)
        .then_some(point)
        .ok_or(InvalidPath::CoordinateRange)
}

fn capsule(a: [f32; 3], b: [f32; 3], radius: f32) -> Result<Tree, InvalidPath> {
    let direction = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
    let length_squared: f32 = direction.iter().map(|n| n * n).sum();
    if !length_squared.is_finite() {
        return Err(InvalidPath::CoordinateRange);
    }
    let p = [Tree::x() - a[0], Tree::y() - a[1], Tree::z() - a[2]];
    let t = if length_squared == 0.0 {
        Tree::constant(0.0)
    } else {
        ((p[0].clone() * direction[0] + p[1].clone() * direction[1] + p[2].clone() * direction[2])
            / length_squared)
            .max(0.0)
            .min(1.0)
    };
    let q = std::array::from_fn::<_, 3, _>(|i| p[i].clone() - t.clone() * direction[i]);
    Ok(q[0].square() + q[1].square() + q[2].square() - radius * radius)
}

impl Sink for Tubes {
    type Error = InvalidPath;

    fn start_at(&mut self, point: Point3) -> Result<(), Self::Error> {
        let point = coordinate(point)?;
        self.paths.extend(self.path.take());
        self.previous = Some(point);
        Ok(())
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        let previous = self.previous.ok_or(InvalidPath::MissingStart)?;
        let point = coordinate(point)?;
        let tube = capsule(previous, point, self.radius)?;
        self.path = Some(match self.path.take() {
            Some(field) => field.min(tube),
            None => tube,
        });
        self.previous = Some(point);
        Ok(())
    }
}

pub(super) fn read_color(value: &Value) -> Option<[u8; 3]> {
    let rgba = color::read(value)?.to_rgba8();
    (rgba.a == 255).then_some([rgba.r, rgba.g, rgba.b])
}

pub(super) fn preview(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    preview_with(
        context,
        call,
        environment,
        PREVIEW_3D,
        fidget::preview_3d_function,
    )
}

pub(super) fn preview_with(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    marker: gid::CellId,
    model: impl FnOnce(&mut Context, Expression, &Environment) -> Result<Value, Halt>,
) -> Result<Value, Halt> {
    result((|| {
        let program = argument(context, call, PROGRAM)?;
        let program = context.eval(program, environment)?;
        if absent::is_absent(&program) {
            return Ok(program);
        }
        let radius = number(context, call, environment, LINE_RADIUS)?;
        read_radius(radius).ok_or_else(invalid)?;
        let color = argument(context, call, fidget::vocabulary::COLOR)?;
        let color = context.eval(color, environment)?;
        read_color(&color).ok_or_else(|| {
            Error::Invalid(absent::with_reason(fidget::vocabulary::INVALID_COLOR))
        })?;
        let fuel = super::fuel(context, call, environment)?;
        let model = model(context, call, environment)?;
        if absent::is_absent(&model) {
            return Ok(model);
        }
        Ok(Value::record([(
            marker,
            Value::record([
                (presentation::vocabulary::VALUE, model),
                (PROGRAM, program),
                (LINE_RADIUS, f64::value(radius)),
                (fidget::vocabulary::COLOR, color),
                (layout::vocabulary::FUEL, f64::value(fuel as f64)),
            ]),
        )]))
    })())
}

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    renderer: &Rc<RefCell<fidget::PreviewRenderer>>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?.get(&PREVIEW_3D)?.as_record()?;
    let model = fidget::volume_preview(fields.get(&presentation::vocabulary::VALUE)?)?;
    let program = fields.get(&PROGRAM)?.clone();
    let radius = f64::read(fields.get(&LINE_RADIUS)?)?;
    read_radius(radius)?;
    let color = read_color(fields.get(&fidget::vocabulary::COLOR)?)?;
    let fuel = f64::read(fields.get(&layout::vocabulary::FUEL)?)?;
    let fuel = super::read_fuel(fuel)?;
    let state = input.state.cloned();
    let scale = input.scale_factor;
    let renderer = renderer.clone();
    let drawing = Layout::program(Rc::new(move |context, build| {
        let mut tubes = Tubes::new(radius).unwrap();
        let evaluation = run(&mut tubes, |scope| {
            ::grap::apply_scoped(&program, [], &context.inputs.sources, scope, fuel)
        });
        if !evaluation.completed || absent::is_absent(&evaluation.result) {
            return context.project.transient(
                context.text,
                build,
                evaluation.result,
                evaluation.remaining_fuel,
            );
        }
        let mut scene = model.clone();
        // Paths go first so exact depth ties do not hide them behind the model.
        scene.objects.splice(
            0..0,
            tubes
                .finish()
                .into_iter()
                .map(|tree| fidget::SceneObject { tree, color }),
        );
        if scene.objects.len() > usize::from(u16::MAX) + 1 {
            return context.project.transient(
                context.text,
                build,
                absent::with_reason(fidget::vocabulary::INVALID_SCENE),
                evaluation.remaining_fuel,
            );
        }
        match fidget::volume_display(&scene, state.as_ref(), scale, &mut renderer.borrow_mut()) {
            Some(drawing) => drawing.measure(context, build),
            None => context.project.transient(
                context.text,
                build,
                absent::with_reason(fidget::vocabulary::INVALID_FIELD),
                evaluation.remaining_fuel,
            ),
        }
    }));
    Some(fidget::interactive_volume(drawing, input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidget_engine::{shape::EzShape, vm::VmShape};

    fn sample(tree: Tree, point: [f32; 3]) -> f32 {
        let shape = VmShape::from(tree);
        let mut evaluator = VmShape::new_float_slice_eval();
        let tape = shape.ez_float_slice_tape();
        evaluator
            .eval(&tape, &[point[0]], &[point[1]], &[point[2]])
            .unwrap()[0]
    }

    #[test]
    fn capsule_has_round_ends_and_the_requested_radius() {
        let tree = capsule([0.0; 3], [1.0, 1.0, 1.0], 0.1).unwrap();
        assert!(sample(tree.clone(), [0.5; 3]) < 0.0);
        assert!(sample(tree.clone(), [0.5, 0.7, 0.5]) > 0.0);
        let end = 1.0 + 0.1 / 3.0_f32.sqrt();
        assert!(sample(tree, [end; 3]).abs() < 1e-6);
        let tree = capsule([0.0; 3], [1.0, 0.0, 0.0], 0.1).unwrap();
        assert!(sample(tree.clone(), [0.5, 0.1, 0.0]).abs() < 1e-6);
        assert!(sample(tree, [-0.1, 0.0, 0.0]).abs() < 1e-6);
    }

    #[test]
    fn tubes_preserve_breaks_and_do_not_draw_isolated_starts() {
        let mut tubes = Tubes::new(0.1).unwrap();
        tubes.start_at([100.0; 3]).unwrap();
        assert!(tubes.path.is_none());
        assert!(tubes.paths.is_empty());
        tubes.start_at([0.0; 3]).unwrap();
        tubes.line_to([1.0, 0.0, 0.0]).unwrap();
        tubes.line_to([2.0, 0.0, 0.0]).unwrap();
        tubes.start_at([0.0, 2.0, 0.0]).unwrap();
        tubes.line_to([1.0, 2.0, 0.0]).unwrap();
        tubes.start_at([100.0; 3]).unwrap();
        let paths = tubes.finish();
        assert_eq!(paths.len(), 2);
        assert!(sample(paths[0].clone(), [0.5, 0.0, 0.0]) < 0.0);
        assert!(sample(paths[0].clone(), [1.5, 0.0, 0.0]) < 0.0);
        assert!(sample(paths[0].clone(), [0.5, 2.0, 0.0]) > 0.0);
        assert!(sample(paths[1].clone(), [0.5, 2.0, 0.0]) < 0.0);
        for path in paths {
            assert!(sample(path, [0.5, 1.0, 0.0]) > 0.0, "no connecting move");
        }
    }

    #[test]
    fn duplicate_points_are_spheres_and_invalid_emissions_leave_the_sink_unchanged() {
        let mut tubes = Tubes::new(0.1).unwrap();
        assert_eq!(tubes.line_to([0.0; 3]), Err(InvalidPath::MissingStart));
        tubes.start_at([0.0; 3]).unwrap();
        tubes.line_to([0.0; 3]).unwrap();
        assert_eq!(
            tubes.start_at([f64::NAN; 3]),
            Err(InvalidPath::NonFinitePoint)
        );
        assert_eq!(
            tubes.line_to([f64::MAX; 3]),
            Err(InvalidPath::CoordinateRange)
        );
        tubes.line_to([0.0; 3]).unwrap();
        let paths = tubes.finish();
        assert_eq!(paths.len(), 1);
        assert!(sample(paths[0].clone(), [0.1, 0.0, 0.0]).abs() < 1e-6);
        for radius in [
            0.0,
            -1.0,
            f64::INFINITY,
            f64::NAN,
            f64::MAX,
            f64::MIN_POSITIVE,
        ] {
            assert!(Tubes::new(radius).is_none());
        }
    }

    #[test]
    fn direct_and_recorded_paths_produce_the_same_fields() {
        fn draw(sink: &mut impl Sink<Error = InvalidPath>) {
            sink.start_at([0.0; 3]).unwrap();
            sink.line_to([1.0, 0.5, 0.1]).unwrap();
            sink.start_at([-1.0, 0.0, 0.5]).unwrap();
            sink.line_to([0.0, 0.5, 0.2]).unwrap();
        }
        let mut direct = Tubes::new(0.1).unwrap();
        draw(&mut direct);
        let mut recording = Recording::default();
        draw(&mut recording);
        let mut replayed = Tubes::new(0.1).unwrap();
        recording.replay(&mut replayed).unwrap();
        assert_eq!(direct.finish(), replayed.finish());
    }
}
