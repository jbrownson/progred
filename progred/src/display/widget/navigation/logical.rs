//! Fold the chosen layout traversal into line boundaries and local neighbors.
use super::{DIRECTIONS, DispatchContext, Event, EventOutcome, Handler, Target};
use gid::Step;
use measured::{Composition, RowAlignment};
use std::{collections::HashMap, rc::Rc};

struct Stop<C> {
    target: Target<C>,
    selected: bool,
}

#[derive(Default)]
struct Neighbors {
    left: Option<usize>,
    right: Option<usize>,
}

/// Concatenation needs only endpoints and the neighbors of a selected stop.
/// Everything in between can be forgotten immediately.
#[derive(Default)]
struct Line {
    first: Option<usize>,
    last: Option<usize>,
    selected: Option<Neighbors>,
    /// A leaf or single-line value, rather than only entrances to a block.
    has_content: bool,
}

impl Line {
    fn stop(stop: usize, selected: bool) -> Self {
        Self {
            first: Some(stop),
            last: Some(stop),
            selected: selected.then(Neighbors::default),
            has_content: true,
        }
    }

    fn append(&mut self, next: Self) {
        if let Some(selected) = &mut self.selected {
            selected.right = selected.right.or(next.first);
        } else if let Some(mut selected) = next.selected {
            selected.left = selected.left.or(self.last);
            self.selected = Some(selected);
        }
        self.first = self.first.or(next.first);
        self.last = next.last.or(self.last);
        self.has_content |= next.has_content;
    }
}

/// Content rows align using the actual column baseline. Each row keeps a small
/// sequence of block-entry/content levels, separate from that content alignment.
/// Consecutive enclosing stops share an entry level, rather than manufacturing
/// a separate vertical step for every wrapper around the same block.
/// These are summaries, not a copy of the chosen layout or a list of stops.
#[derive(Default)]
struct Lines {
    baseline: usize,
    rows: Vec<Vec<Line>>,
}

impl Lines {
    fn stop(stop: usize, selected: bool) -> Self {
        Self {
            baseline: 0,
            rows: vec![vec![Line::stop(stop, selected)]],
        }
    }

    fn merge_at(&mut self, next: Self, offset: usize) {
        if next.rows.is_empty() {
            return;
        }
        self.rows
            .resize_with(self.rows.len().max(offset + next.rows.len()), Vec::new);
        for (row, levels) in next.rows.into_iter().enumerate() {
            let target = &mut self.rows[offset + row];
            target.resize_with(target.len().max(levels.len()), Line::default);
            for (target, next) in target.iter_mut().zip(levels) {
                target.append(next);
            }
        }
    }

    fn enclosed(mut self, stop: usize, selected: bool) -> Self {
        if self.rows.is_empty() {
            return Self::stop(stop, selected);
        }
        let mut entry = Line::stop(stop, selected);
        if self.rows.len() > 1 {
            entry.has_content = false;
            if self.rows[0].first().is_some_and(|line| !line.has_content) {
                entry.append(std::mem::take(&mut self.rows[0][0]));
                self.rows[0][0] = entry;
            } else {
                self.rows[0].insert(0, entry);
            }
        } else {
            entry.append(std::mem::take(&mut self.rows[0][0]));
            self.rows[0][0] = entry;
        }
        self
    }

    fn destinations(self) -> Option<[Option<usize>; 4]> {
        let (mut previous_first, mut previous_last) = (None, None);
        let mut result: Option<[Option<usize>; 4]> = None;
        for line in self
            .rows
            .into_iter()
            .flatten()
            .filter(|line| line.first.is_some())
        {
            if let Some([_, right, _, down]) = &mut result {
                *right = right.or(line.first);
                *down = line.first;
                break;
            }
            if let Some(selected) = line.selected {
                result = Some([
                    selected.left.or(previous_last),
                    selected.right,
                    previous_first,
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
struct Accumulator {
    composition: Composition,
    child_index: usize,
    lines: Lines,
}

impl Default for Accumulator {
    fn default() -> Self {
        Self::new(Composition::Row(RowAlignment::Baseline))
    }
}

impl Accumulator {
    fn new(composition: Composition) -> Self {
        Self {
            composition,
            child_index: 0,
            lines: Lines::default(),
        }
    }

    fn push(&mut self, child: Lines) {
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
                    self.lines.rows.resize_with(length + offset, Vec::new);
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

// Canonical targets let a later declaration refine arrival behavior without
// creating a second navigation position at the same displayed path.
pub(crate) struct Construction<C> {
    stops: Vec<Stop<C>>,
    occurrences: HashMap<Rc<[Step]>, usize>,
    current: Accumulator,
    parents: Vec<Accumulator>,
}

impl<C> Default for Construction<C> {
    fn default() -> Self {
        Self {
            stops: Vec::new(),
            occurrences: HashMap::new(),
            current: Accumulator::default(),
            parents: Vec::new(),
        }
    }
}

impl<C: 'static> Construction<C> {
    fn register(&mut self, target: Target<C>, selected: bool) -> Option<usize> {
        let stop = Stop { target, selected };
        match self.occurrences.get(&stop.target.path) {
            Some(&index) => {
                self.stops[index] = stop;
                None
            }
            None => {
                let index = self.stops.len();
                self.occurrences.insert(stop.target.path.clone(), index);
                self.stops.push(stop);
                Some(index)
            }
        }
    }

    pub fn target(&mut self, target: Target<C>, selected: bool) {
        if let Some(index) = self.register(target, selected) {
            self.current.push(Lines::stop(index, selected));
        }
    }

    pub fn begin(&mut self, composition: Composition) {
        self.parents.push(std::mem::replace(
            &mut self.current,
            Accumulator::new(composition),
        ));
    }

    fn close(&mut self) -> Lines {
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

    pub fn begin_container(&mut self, target: Target<C>, selected: bool) -> Option<usize> {
        let stop = self.register(target, selected);
        self.begin(Composition::Row(RowAlignment::Baseline));
        stop
    }

    pub fn end_container(&mut self, stop: Option<usize>) {
        let lines = self.close();
        self.current.push(match stop {
            Some(stop) => lines.enclosed(stop, self.stops[stop].selected),
            None => lines,
        });
    }

    pub fn finish<H: 'static>(self) -> Option<Handler<C, DispatchContext<C, H>>> {
        debug_assert!(self.parents.is_empty());
        let destinations = self
            .current
            .lines
            .destinations()?
            .map(|index| index.map(|index| self.stops[index].target.clone()));
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
