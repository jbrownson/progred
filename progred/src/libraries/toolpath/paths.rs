//! Streaming path geometry. Starting a path never implies a connecting move.

pub type Point3 = [f64; 3];

pub trait Sink {
    type Error;

    fn start_at(&mut self, point: Point3) -> Result<(), Self::Error>;
    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error>;
}

pub struct MapPoints<S, F> {
    pub sink: S,
    pub map: F,
}

impl<S: Sink, F: FnMut(Point3) -> Result<Point3, S::Error>> Sink for MapPoints<S, F> {
    type Error = S::Error;

    fn start_at(&mut self, point: Point3) -> Result<(), Self::Error> {
        self.sink.start_at((self.map)(point)?)
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        self.sink.line_to((self.map)(point)?)
    }
}

impl<S: Sink + ?Sized> Sink for &mut S {
    type Error = S::Error;

    fn start_at(&mut self, point: Point3) -> Result<(), Self::Error> {
        (**self).start_at(point)
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        (**self).line_to(point)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    StartAt(Point3),
    LineTo(Point3),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidPath {
    NonFinitePoint,
    MissingStart,
    CoordinateRange,
}

#[derive(Default)]
pub struct Recording {
    pub commands: Vec<Command>,
}

impl Recording {
    pub fn segments(&self) -> impl Iterator<Item = (Point3, Point3)> + '_ {
        let mut previous = None;
        self.commands
            .iter()
            .filter_map(move |command| match *command {
                Command::StartAt(point) => {
                    previous = Some(point);
                    None
                }
                Command::LineTo(point) => previous.replace(point).map(|start| (start, point)),
            })
    }

    /// Cutting distance only: starts carry no implicit rapid or linking motion.
    pub fn length(&self) -> Result<f64, InvalidPath> {
        let length = self.segments().map(|(a, b)| distance(a, b)).sum::<f64>();
        length
            .is_finite()
            .then_some(length)
            .ok_or(InvalidPath::CoordinateRange)
    }

    /// Emit complete and upcoming segments, splitting the segment at the cursor.
    pub fn playback(
        &self,
        progress: f64,
        mut emit: impl FnMut(Point3, Point3, bool) -> Result<(), InvalidPath>,
    ) -> Result<Option<Point3>, InvalidPath> {
        if !progress.is_finite() {
            return Err(InvalidPath::NonFinitePoint);
        }
        let mut remaining = progress.clamp(0.0, 1.0) * self.length()?;
        let mut position = None;
        for (a, b) in self.segments() {
            position.get_or_insert(a);
            let length = distance(a, b);
            if progress >= 1.0 || (remaining >= length && remaining > 0.0) {
                emit(a, b, true)?;
                remaining = (remaining - length).max(0.0);
                position = Some(b);
            } else if remaining > 0.0 {
                let t = remaining / length;
                let point = std::array::from_fn(|i| a[i] + t * (b[i] - a[i]));
                emit(a, point, true)?;
                emit(point, b, false)?;
                position = Some(point);
                remaining = 0.0;
            } else {
                emit(a, b, false)?;
            }
        }
        Ok(position)
    }

    #[cfg(test)]
    pub fn replay<S: Sink + ?Sized>(&self, sink: &mut S) -> Result<(), S::Error> {
        for command in &self.commands {
            match *command {
                Command::StartAt(point) => sink.start_at(point)?,
                Command::LineTo(point) => sink.line_to(point)?,
            }
        }
        Ok(())
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
        path.start_at([0.0, 0.0, 0.0]).unwrap();
        path.line_to([1.0, 0.0, 0.0]).unwrap();
        path.start_at([10.0, 0.0, 0.0]).unwrap();
        path.line_to([13.0, 0.0, 0.0]).unwrap();
        assert_eq!(path.length().unwrap(), 4.0);
        for (progress, expected) in [(0.0, 0.0), (0.25, 1.0), (0.5, 11.0), (1.0, 13.0)] {
            let mut completed = 0.0;
            let position = path
                .playback(progress, |a, b, done| {
                    assert!(distance(a, b) <= 3.0);
                    if done {
                        completed += distance(a, b);
                    }
                    Ok(())
                })
                .unwrap()
                .unwrap();
            assert_eq!(position, [expected, 0.0, 0.0]);
            assert_eq!(completed, progress * 4.0);
        }
    }

    #[test]
    fn empty_zero_length_and_overflow_are_explicit() {
        let mut path = Recording::default();
        assert_eq!(path.playback(0.5, |_, _, _| Ok(())).unwrap(), None);
        path.start_at([2.0; 3]).unwrap();
        path.line_to([2.0; 3]).unwrap();
        assert_eq!(
            path.playback(0.5, |_, _, _| Ok(())).unwrap(),
            Some([2.0; 3])
        );
        path.start_at([-f64::MAX; 3]).unwrap();
        path.line_to([f64::MAX; 3]).unwrap();
        assert!(path.length().is_err());
    }
}

impl Sink for Recording {
    type Error = InvalidPath;

    fn start_at(&mut self, point: Point3) -> Result<(), Self::Error> {
        if !point.into_iter().all(f64::is_finite) {
            return Err(InvalidPath::NonFinitePoint);
        }
        self.commands.push(Command::StartAt(point));
        Ok(())
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        if self.commands.is_empty() {
            return Err(InvalidPath::MissingStart);
        }
        if !point.into_iter().all(f64::is_finite) {
            return Err(InvalidPath::NonFinitePoint);
        }
        self.commands.push(Command::LineTo(point));
        Ok(())
    }
}
