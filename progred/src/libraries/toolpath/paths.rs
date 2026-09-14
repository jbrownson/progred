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

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    StartAt(Point3),
    LineTo(Point3),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidPath {
    NonFinitePoint,
    MissingStart,
}

#[cfg(test)]
#[derive(Default)]
pub struct Recording {
    pub commands: Vec<Command>,
}

#[cfg(test)]
impl Recording {
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

#[cfg(test)]
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
