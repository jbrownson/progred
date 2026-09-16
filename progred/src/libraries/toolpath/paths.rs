//! Streaming path geometry. Starting a path never implies a connecting move.

use super::cutter::Tool;

pub type Point3 = [f64; 3];

/// Unit vector from the tool tip toward the spindle. A path has one fixed
/// axis (3+2 positioning); changing orientation requires starting a new path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis(Point3);

impl Axis {
    pub const Z: Self = Self([0.0, 0.0, 1.0]);

    pub fn new(vector: Point3) -> Option<Self> {
        let scale = vector.into_iter().map(f64::abs).fold(0.0, f64::max);
        if !vector.into_iter().all(f64::is_finite) || scale == 0.0 {
            return None;
        }
        let scaled = vector.map(|v| v / scale);
        let length = scaled[0].hypot(scaled[1]).hypot(scaled[2]);
        Some(Self(scaled.map(|v| v / length)))
    }

    pub fn vector(self) -> Point3 {
        self.0
    }

    pub fn basis(self) -> [nalgebra::Vector3<f32>; 3] {
        use nalgebra::Vector3;
        let z = Vector3::from(self.vector().map(|v| v as f32)).normalize();
        let helper = if z.x.abs() < 0.9 {
            Vector3::x()
        } else {
            Vector3::y()
        };
        let y = z.cross(&helper).normalize();
        let x = y.cross(&z);
        [x, y, z]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub tip: Point3,
    pub axis: Axis,
}

pub trait Sink {
    type Error;

    fn start_at(&mut self, point: Point3, axis: Axis) -> Result<(), Self::Error>;
    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error>;

    /// End the current path, without emitting a connecting move.
    fn end_path(&mut self);

    /// Balanced scope notifications. Geometry-only consumers ignore the tool,
    /// but must still respect the path boundary. Use `with_tool` for native code.
    fn enter_tool(&mut self, _tool: &Tool) {
        self.end_path();
    }
    fn leave_tool(&mut self) {
        self.end_path();
    }
}

pub fn with_tool<S: Sink + ?Sized, T>(
    sink: &mut S,
    tool: &Tool,
    body: impl FnOnce(&mut S) -> T,
) -> T {
    sink.enter_tool(tool);
    let result = body(sink);
    sink.leave_tool();
    result
}

pub struct MapPoints<S, F> {
    pub sink: S,
    pub map: F,
}

impl<S: Sink, F: FnMut(Point3) -> Result<Point3, S::Error>> Sink for MapPoints<S, F> {
    type Error = S::Error;

    fn start_at(&mut self, point: Point3, axis: Axis) -> Result<(), Self::Error> {
        self.sink.start_at((self.map)(point)?, axis)
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        self.sink.line_to((self.map)(point)?)
    }

    fn end_path(&mut self) {
        self.sink.end_path();
    }
    fn enter_tool(&mut self, tool: &Tool) {
        self.sink.enter_tool(tool);
    }
    fn leave_tool(&mut self) {
        self.sink.leave_tool();
    }
}

impl<S: Sink + ?Sized> Sink for &mut S {
    type Error = S::Error;

    fn start_at(&mut self, point: Point3, axis: Axis) -> Result<(), Self::Error> {
        (**self).start_at(point, axis)
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        (**self).line_to(point)
    }

    fn end_path(&mut self) {
        (**self).end_path();
    }
    fn enter_tool(&mut self, tool: &Tool) {
        (**self).enter_tool(tool);
    }
    fn leave_tool(&mut self) {
        (**self).leave_tool();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    StartAt(Point3, Axis),
    LineTo(Point3),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidPath {
    NonFinitePoint,
    MissingStart,
    CoordinateRange,
    MissingTool,
}

#[derive(Debug, PartialEq)]
pub enum Entry {
    Move(Command),
    WithTool(Tool, Vec<Entry>),
}

#[derive(Default, Debug, PartialEq)]
pub struct Recording {
    pub entries: Vec<Entry>,
    scopes: Vec<(Tool, Vec<Entry>)>,
    started: bool,
}

impl Recording {
    /// Visit moves with their enclosing tool, without allocating a flat copy.
    pub fn moves(&self) -> impl Iterator<Item = (Command, Option<&Tool>)> + '_ {
        let mut stack = vec![(self.entries.iter(), None)];
        std::iter::from_fn(move || {
            loop {
                let (entries, tool) = stack.last_mut()?;
                match entries.next() {
                    Some(Entry::Move(command)) => return Some((*command, *tool)),
                    Some(Entry::WithTool(tool, children)) => {
                        stack.push((children.iter(), Some(tool)))
                    }
                    None => {
                        stack.pop();
                    }
                }
            }
        })
    }

    pub fn segments(&self) -> impl Iterator<Item = (Point3, Point3, Axis)> + '_ {
        self.tool_segments().map(|(a, b, axis, _)| (a, b, axis))
    }

    pub fn tool_segments(
        &self,
    ) -> impl Iterator<Item = (Point3, Point3, Axis, Option<&Tool>)> + '_ {
        let mut previous = None;
        let mut axis = Axis::Z;
        self.moves()
            .filter_map(move |(command, tool)| match command {
                Command::StartAt(point, next_axis) => {
                    axis = next_axis;
                    previous = Some(point);
                    None
                }
                Command::LineTo(point) => previous
                    .replace(point)
                    .map(|start| (start, point, axis, tool)),
            })
    }

    /// Cutting distance only: starts carry no implicit rapid or linking motion.
    pub fn length(&self) -> Result<f64, InvalidPath> {
        let length = self.segments().map(|(a, b, _)| distance(a, b)).sum::<f64>();
        length
            .is_finite()
            .then_some(length)
            .ok_or(InvalidPath::CoordinateRange)
    }

    /// Emit complete and upcoming segments, splitting the segment at the cursor.
    pub fn playback<E: From<InvalidPath>>(
        &self,
        progress: f64,
        mut emit: impl FnMut(Point3, Point3, Axis, Option<&Tool>, bool) -> Result<(), E>,
    ) -> Result<Option<(Pose, Option<&Tool>)>, E> {
        if !progress.is_finite() {
            return Err(InvalidPath::NonFinitePoint.into());
        }
        let mut remaining = progress.clamp(0.0, 1.0) * self.length()?;
        let mut position = None;
        for (a, b, axis, tool) in self.tool_segments() {
            position.get_or_insert((Pose { tip: a, axis }, tool));
            let length = distance(a, b);
            if progress >= 1.0 || (remaining >= length && remaining > 0.0) {
                emit(a, b, axis, tool, true)?;
                remaining = (remaining - length).max(0.0);
                position = Some((Pose { tip: b, axis }, tool));
            } else if remaining > 0.0 {
                let t = remaining / length;
                let point = std::array::from_fn(|i| a[i] + t * (b[i] - a[i]));
                emit(a, point, axis, tool, true)?;
                emit(point, b, axis, tool, false)?;
                position = Some((Pose { tip: point, axis }, tool));
                remaining = 0.0;
            } else {
                emit(a, b, axis, tool, false)?;
            }
        }
        Ok(position)
    }

    pub fn replay<S: Sink + ?Sized>(&self, sink: &mut S) -> Result<(), S::Error> {
        fn replay<S: Sink + ?Sized>(entries: &[Entry], sink: &mut S) -> Result<(), S::Error> {
            for entry in entries {
                match entry {
                    Entry::Move(Command::StartAt(point, axis)) => sink.start_at(*point, *axis)?,
                    Entry::Move(Command::LineTo(point)) => sink.line_to(*point)?,
                    Entry::WithTool(tool, children) => {
                        with_tool(sink, tool, |sink| replay(children, sink))?
                    }
                }
            }
            Ok(())
        }
        replay(&self.entries, sink)
    }
}

fn distance(a: Point3, b: Point3) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1]).hypot(b[2] - a[2])
}

#[cfg(test)]
mod playback_tests {
    use super::*;

    #[test]
    fn seeking_is_by_distance_and_never_draws_across_path_breaks() {
        let mut path = Recording::default();
        path.start_at([0.0, 0.0, 0.0], Axis::Z).unwrap();
        path.line_to([1.0, 0.0, 0.0]).unwrap();
        path.start_at([10.0, 0.0, 0.0], Axis::Z).unwrap();
        path.line_to([13.0, 0.0, 0.0]).unwrap();
        assert_eq!(path.length().unwrap(), 4.0);
        for (progress, expected) in [(0.0, 0.0), (0.25, 1.0), (0.5, 11.0), (1.0, 13.0)] {
            let mut completed = 0.0;
            let position = path
                .playback::<InvalidPath>(progress, |a, b, _, _, done| {
                    assert!(distance(a, b) <= 3.0);
                    if done {
                        completed += distance(a, b);
                    }
                    Ok(())
                })
                .unwrap()
                .unwrap();
            assert_eq!(
                position.0,
                Pose {
                    tip: [expected, 0.0, 0.0],
                    axis: Axis::Z
                }
            );
            assert_eq!(completed, progress * 4.0);
        }
    }

    #[test]
    fn playback_preserves_consumer_errors_and_stops_emitting() {
        #[derive(Debug, PartialEq)]
        enum Error {
            Path(InvalidPath),
            Consumer,
        }
        impl From<InvalidPath> for Error {
            fn from(error: InvalidPath) -> Self {
                Self::Path(error)
            }
        }
        let mut path = Recording::default();
        path.start_at([0.0; 3], Axis::Z).unwrap();
        path.line_to([1.0; 3]).unwrap();
        path.line_to([2.0; 3]).unwrap();
        let mut calls = 0;
        let result = path.playback(1.0, |_, _, _, _, _| {
            calls += 1;
            Err(Error::Consumer)
        });
        assert_eq!(result, Err(Error::Consumer));
        assert_eq!(calls, 1);
        assert_eq!(
            path.playback::<Error>(f64::NAN, |_, _, _, _, _| panic!(
                "invalid progress must not emit"
            )),
            Err(Error::Path(InvalidPath::NonFinitePoint)),
        );
    }

    #[test]
    fn empty_zero_length_and_overflow_are_explicit() {
        let mut path = Recording::default();
        assert_eq!(
            path.playback::<InvalidPath>(0.5, |_, _, _, _, _| Ok(()))
                .unwrap(),
            None
        );
        path.start_at([2.0; 3], Axis::Z).unwrap();
        path.line_to([2.0; 3]).unwrap();
        assert_eq!(
            path.playback::<InvalidPath>(0.5, |_, _, _, _, _| Ok(()))
                .unwrap(),
            Some((
                Pose {
                    tip: [2.0; 3],
                    axis: Axis::Z
                },
                None
            ))
        );
        path.start_at([-f64::MAX; 3], Axis::Z).unwrap();
        path.line_to([f64::MAX; 3]).unwrap();
        assert!(path.length().is_err());
    }
}

impl Sink for Recording {
    type Error = InvalidPath;

    fn start_at(&mut self, point: Point3, axis: Axis) -> Result<(), Self::Error> {
        if !point.into_iter().all(f64::is_finite) {
            return Err(InvalidPath::NonFinitePoint);
        }
        self.entries
            .push(Entry::Move(Command::StartAt(point, axis)));
        self.started = true;
        Ok(())
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        if !self.started {
            return Err(InvalidPath::MissingStart);
        }
        if !point.into_iter().all(f64::is_finite) {
            return Err(InvalidPath::NonFinitePoint);
        }
        self.entries.push(Entry::Move(Command::LineTo(point)));
        Ok(())
    }

    fn end_path(&mut self) {
        self.started = false;
    }

    fn enter_tool(&mut self, tool: &Tool) {
        self.end_path();
        self.scopes
            .push((tool.clone(), std::mem::take(&mut self.entries)));
    }

    fn leave_tool(&mut self) {
        self.end_path();
        let (tool, parent) = self.scopes.pop().expect("balanced tool scope");
        let children = std::mem::replace(&mut self.entries, parent);
        self.entries.push(Entry::WithTool(tool, children));
    }
}
