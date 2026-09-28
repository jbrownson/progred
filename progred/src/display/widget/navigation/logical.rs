//! Fold the chosen layout traversal into line boundaries and local neighbors.
use super::{DIRECTIONS, DispatchContext, Event, EventOutcome, Handler, Target};
use measured::{Composition, RowAlignment};

struct Stop<C> {
    target: Target<C>,
    selected: bool,
}

struct Neighbors<C> {
    left: Option<Target<C>>,
    right: Option<Target<C>>,
}

/// Concatenation needs only endpoints and the neighbors of a selected stop.
/// Everything in between is dropped as soon as it is passed.
struct Line<C> {
    first: Option<Target<C>>,
    last: Option<Target<C>>,
    selected: Option<Neighbors<C>>,
}

impl<C> Default for Line<C> {
    fn default() -> Self {
        Self {
            first: None,
            last: None,
            selected: None,
        }
    }
}

impl<C> Line<C> {
    fn stop(stop: Stop<C>) -> Self {
        Self {
            first: Some(stop.target.clone()),
            selected: stop.selected.then_some(Neighbors {
                left: None,
                right: None,
            }),
            last: Some(stop.target),
        }
    }

    fn append(&mut self, next: Self) {
        match (&mut self.selected, next.selected) {
            (Some(selected), _) => {
                selected.right = selected.right.take().or_else(|| next.first.clone());
            }
            (None, Some(mut selected)) => {
                selected.left = selected.left.or_else(|| self.last.clone());
                self.selected = Some(selected);
            }
            (None, None) => {}
        }
        self.first = self.first.take().or(next.first);
        self.last = next.last.or(self.last.take());
    }
}

/// One content row. `levels` exist once a multiline block begins on this row:
/// its entry level, then the lines it draws. Stops before the first block
/// join its entry line; stops after it stay on the line they are drawn on.
struct Row<C> {
    before: Line<C>,
    levels: Vec<Line<C>>,
}

impl<C> Default for Row<C> {
    fn default() -> Self {
        Self {
            before: Line::default(),
            levels: Vec::new(),
        }
    }
}

impl<C> Row<C> {
    fn append(&mut self, next: Self) {
        match self.levels.last_mut() {
            None => {
                self.before.append(next.before);
                self.levels = next.levels;
            }
            Some(drawn) => {
                drawn.append(next.before);
                // Blocks side by side share their entry and drawn lines.
                let skip = self.levels.len().saturating_sub(next.levels.len());
                self.levels.resize_with(
                    self.levels.len().max(skip + next.levels.len()),
                    Line::default,
                );
                for (level, next) in self.levels.iter_mut().skip(skip).zip(next.levels) {
                    level.append(next);
                }
            }
        }
    }

    fn lines(self) -> impl Iterator<Item = Line<C>> {
        let mut levels = self.levels.into_iter();
        let mut first = self.before;
        if let Some(entry) = levels.next() {
            first.append(entry);
        }
        std::iter::once(first).chain(levels)
    }
}

/// Content rows align using the actual column baseline; block entry levels
/// stay inside each row. These are summaries, not a copy of the layout.
struct Lines<C> {
    baseline: usize,
    rows: Vec<Row<C>>,
}

impl<C> Default for Lines<C> {
    fn default() -> Self {
        Self {
            baseline: 0,
            rows: Vec::new(),
        }
    }
}

impl<C> Lines<C> {
    fn stop(stop: Stop<C>) -> Self {
        Self {
            baseline: 0,
            rows: vec![Row {
                before: Line::stop(stop),
                levels: Vec::new(),
            }],
        }
    }

    fn merge_at(&mut self, next: Self, offset: usize) {
        if !next.rows.is_empty() {
            self.rows
                .resize_with(self.rows.len().max(offset + next.rows.len()), Row::default);
            for (row, next) in self.rows[offset..].iter_mut().zip(next.rows) {
                row.append(next);
            }
        }
    }

    /// A whole-value stop precedes its contents. Multiline contents are a
    /// block: its stop gets an entry line, shared with directly enclosed
    /// blocks. Single-line contents stay on their line.
    fn enclosed(mut self, stop: Stop<C>) -> Self {
        let multiline = self.rows.len() > 1;
        match self.rows.first_mut() {
            None => Self::stop(stop),
            Some(row) => {
                let mut entry = Line::stop(stop);
                if !multiline && row.levels.is_empty() {
                    entry.append(std::mem::take(&mut row.before));
                    row.before = entry;
                } else if row.before.first.is_none() && !row.levels.is_empty() {
                    entry.append(std::mem::take(&mut row.levels[0]));
                    row.levels[0] = entry;
                } else {
                    let mut drawn = std::mem::take(&mut row.before);
                    if !row.levels.is_empty() {
                        drawn.append(row.levels.remove(0));
                    }
                    row.levels.splice(0..0, [entry, drawn]);
                }
                self
            }
        }
    }

    fn destinations(self) -> Option<[Option<Target<C>>; 4]> {
        let (mut previous_first, mut previous_last) = (None, None);
        let mut result: Option<[Option<Target<C>>; 4]> = None;
        for line in self
            .rows
            .into_iter()
            .flat_map(Row::lines)
            .filter(|line| line.first.is_some())
        {
            if let Some([_, right, _, down]) = &mut result {
                *right = right.take().or_else(|| line.first.clone());
                *down = line.first;
                break;
            }
            if let Some(selected) = line.selected {
                result = Some([
                    selected.left.or(previous_last.take()),
                    selected.right,
                    previous_first.take(),
                    None,
                ]);
            }
            previous_first = line.first;
            previous_last = line.last;
        }
        result
    }
}

/// One open row/column. Children are folded as they finish, so a long row
/// does not retain a summary for each of its already-visited children.
struct Accumulator<C> {
    composition: Composition,
    child_index: usize,
    lines: Lines<C>,
}

impl<C> Default for Accumulator<C> {
    fn default() -> Self {
        Self::new(Composition::Row(RowAlignment::Baseline))
    }
}

impl<C> Accumulator<C> {
    fn new(composition: Composition) -> Self {
        Self {
            composition,
            child_index: 0,
            lines: Lines::default(),
        }
    }

    fn push(&mut self, child: Lines<C>) {
        if self.child_index == 0 {
            self.lines = child;
            if !matches!(
                self.composition,
                Composition::Row(RowAlignment::Baseline | RowAlignment::Top { baseline: 0 })
                    | Composition::Column { baseline: 0 }
                    | Composition::Overlay
            ) {
                self.lines.baseline = 0;
            }
            self.child_index = 1;
            return;
        }
        match self.composition {
            Composition::Row(RowAlignment::Baseline) | Composition::Overlay => {
                let baseline = self.lines.baseline.max(child.baseline);
                if baseline > self.lines.baseline && !self.lines.rows.is_empty() {
                    let offset = baseline - self.lines.baseline;
                    let length = self.lines.rows.len();
                    self.lines.rows.resize_with(length + offset, Row::default);
                    self.lines.rows.rotate_right(offset);
                }
                self.lines.baseline = baseline;
                let offset = baseline - child.baseline;
                self.lines.merge_at(child, offset);
            }
            Composition::Row(alignment) => {
                if let RowAlignment::Top { baseline } = alignment {
                    if self.child_index == baseline {
                        self.lines.baseline = child.baseline;
                    }
                }
                self.lines.merge_at(child, 0);
            }
            Composition::Column { baseline } => {
                if self.child_index == baseline {
                    self.lines.baseline = self.lines.rows.len() + child.baseline;
                }
                self.lines.rows.extend(child.rows);
            }
        }
        self.child_index += 1;
    }
}

pub(crate) struct Construction<C> {
    current: Accumulator<C>,
    parents: Vec<Accumulator<C>>,
    /// Open whole-value stops; `None` marks one that refined an outer stop.
    containers: Vec<Option<Stop<C>>>,
}

impl<C> Default for Construction<C> {
    fn default() -> Self {
        Self {
            current: Accumulator::default(),
            parents: Vec::new(),
            containers: Vec::new(),
        }
    }
}

impl<C: 'static> Construction<C> {
    /// A repeated declaration of the innermost open whole value's occurrence
    /// refines its arrival behavior instead of adding a second stop.
    fn refine(&mut self, stop: Stop<C>) -> Option<Stop<C>> {
        match self.containers.iter_mut().rev().flatten().next() {
            Some(open) if open.target.path == stop.target.path => {
                *open = stop;
                None
            }
            _ => Some(stop),
        }
    }

    pub fn target(&mut self, target: Target<C>, selected: bool) {
        if let Some(stop) = self.refine(Stop { target, selected }) {
            self.current.push(Lines::stop(stop));
        }
    }

    pub fn begin(&mut self, composition: Composition) {
        self.parents.push(std::mem::replace(
            &mut self.current,
            Accumulator::new(composition),
        ));
    }

    fn close(&mut self) -> Lines<C> {
        let parent = self
            .parents
            .pop()
            .expect("balanced chosen-layout traversal");
        std::mem::replace(&mut self.current, parent).lines
    }

    pub fn end(&mut self) {
        let lines = self.close();
        self.current.push(lines);
    }

    pub fn begin_container(&mut self, target: Target<C>, selected: bool) {
        let stop = self.refine(Stop { target, selected });
        self.containers.push(stop);
        self.begin(Composition::Row(RowAlignment::Baseline));
    }

    pub fn end_container(&mut self) {
        let lines = self.close();
        self.current.push(match self.containers.pop().flatten() {
            Some(stop) => lines.enclosed(stop),
            None => lines,
        });
    }

    pub fn finish<H: 'static>(self) -> Option<Handler<C, DispatchContext<C, H>>> {
        debug_assert!(self.parents.is_empty() && self.containers.is_empty());
        let destinations = self.current.lines.destinations()?;
        let mut handler = Handler::new();
        handler.on(move |world, event, _| match event {
            Event::Navigate(direction) => {
                let target = DIRECTIONS
                    .iter()
                    .position(|d| *d == direction)
                    .and_then(|index| destinations[index].as_ref());
                EventOutcome::from_handled(
                    event,
                    target.is_some_and(|target| (target.select)(world, Some(direction))),
                )
            }
            _ => EventOutcome::decline(event),
        });
        Some(handler)
    }
}

#[cfg(test)]
mod tests;
