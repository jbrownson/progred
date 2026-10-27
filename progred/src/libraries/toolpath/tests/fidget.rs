use super::super::cutter::{self, Tool};
use super::playback::Draw;
use super::*;
use cam::field::{capsule, coordinate};
use fidget_engine::{context::Tree, shape::EzShape, vm::VmShape};

// The all-implicit path field: the benchmark reference, and the subject of a
// regression test that it compiles without GPU memory operations.
pub(crate) struct Tubes {
    radius: f32,
    previous: Option<[f32; 3]>,
    path: Option<Tree>,
    paths: Vec<Tree>,
    color: [u8; 3],
    objects: Vec<volume::SceneObject>,
}

impl Tubes {
    pub(crate) fn new(radius: f64) -> Option<Self> {
        Some(Self {
            radius: read_radius(radius)?,
            previous: None,
            path: None,
            paths: Vec::new(),
            color: [255; 3],
            objects: Vec::new(),
        })
    }

    fn flush(&mut self) {
        self.paths.extend(self.path.take());
        self.objects
            .extend(self.paths.drain(..).map(|tree| volume::SceneObject {
                tree,
                color: self.color,
            }));
    }

    pub(crate) fn scene(mut self) -> Vec<volume::SceneObject> {
        self.flush();
        self.objects
    }
}

impl Draw for Tubes {
    fn style(&mut self, radius: f64, color: [u8; 3]) -> Result<(), InvalidPath> {
        let radius = read_radius(radius).ok_or(InvalidPath::CoordinateRange)?;
        if self.radius != radius || self.color != color {
            self.flush();
            self.radius = radius;
            self.color = color;
        }
        Ok(())
    }

    fn tool(&mut self, tool: &Tool, pose: Pose, tolerance: f64) -> Result<(), InvalidPath> {
        self.flush();
        for section in tool.sections() {
            self.objects.push(volume::SceneObject {
                tree: section.sweep(tolerance, pose.tip, pose.tip, pose.axis)?,
                color: cutter::color(section.kind),
            });
        }
        Ok(())
    }
}

impl Sink for Tubes {
    type Error = InvalidPath;

    fn end_path(&mut self) {
        self.paths.extend(self.path.take());
        self.previous = None;
    }

    fn start_at(&mut self, point: Point3, _: Axis) -> Result<(), Self::Error> {
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

fn sample(tree: Tree, point: [f32; 3]) -> f32 {
    let shape = VmShape::from(tree);
    let mut evaluator = VmShape::new_float_slice_eval();
    let tape = shape.ez_float_slice_tape();
    evaluator
        .eval(&tape, &[point[0]], &[point[1]], &[point[2]])
        .unwrap()[0]
}

#[test]
fn tubes_preserve_breaks_and_do_not_draw_isolated_starts() {
    let mut tubes = Tubes::new(0.1).unwrap();
    tubes.start_at([100.0; 3], Axis::Z).unwrap();
    assert!(tubes.path.is_none());
    assert!(tubes.paths.is_empty());
    tubes.start_at([0.0; 3], Axis::Z).unwrap();
    tubes.line_to([1.0, 0.0, 0.0]).unwrap();
    tubes.line_to([2.0, 0.0, 0.0]).unwrap();
    tubes.start_at([0.0, 2.0, 0.0], Axis::Z).unwrap();
    tubes.line_to([1.0, 2.0, 0.0]).unwrap();
    tubes.start_at([100.0; 3], Axis::Z).unwrap();
    let paths = tubes
        .scene()
        .into_iter()
        .map(|object| object.tree)
        .collect::<Vec<_>>();
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
    tubes.start_at([0.0; 3], Axis::Z).unwrap();
    tubes.line_to([0.0; 3]).unwrap();
    assert_eq!(
        tubes.start_at([f64::NAN; 3], Axis::Z),
        Err(InvalidPath::NonFinitePoint)
    );
    assert_eq!(
        tubes.line_to([f64::MAX; 3]),
        Err(InvalidPath::CoordinateRange)
    );
    tubes.line_to([0.0; 3]).unwrap();
    let paths = tubes
        .scene()
        .into_iter()
        .map(|object| object.tree)
        .collect::<Vec<_>>();
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
        sink.start_at([0.0; 3], Axis::Z).unwrap();
        sink.line_to([1.0, 0.5, 0.1]).unwrap();
        sink.start_at([-1.0, 0.0, 0.5], Axis::Z).unwrap();
        sink.line_to([0.0, 0.5, 0.2]).unwrap();
    }
    let mut direct = Tubes::new(0.1).unwrap();
    draw(&mut direct);
    let mut recording = Recording::default();
    draw(&mut recording);
    let mut replayed = Tubes::new(0.1).unwrap();
    recording.replay(&mut replayed).unwrap();
    assert!(direct.scene() == replayed.scene());
}
