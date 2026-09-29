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

/// One content row: `lead` holds the stops before its first multiline block
/// (all of them if there is none). Stops before the first block join its entry
/// line; stops after it stay on the line they are drawn on.
struct Row<C> {
    lead: Line<C>,
    block: Option<Block<C>>,
}

/// Lines count up from the drawn line, so blocks side by side combine line by
/// line however their row is grouped. `attach` is the entry line of the first
/// block, which the row's lead joins.
struct Block<C> {
    drawn: Line<C>,
    entries: Vec<Line<C>>,
    attach: usize,
}

impl<C> Default for Row<C> {
    fn default() -> Self {
        Self {
            lead: Line::default(),
            block: None,
        }
    }
}

impl<C> Row<C> {
    fn append(&mut self, next: Self) {
        match (&mut self.block, next.block) {
            (None, block) => {
                self.lead.append(next.lead);
                self.block = block;
            }
            (Some(block), None) => block.drawn.append(next.lead),
            (Some(block), Some(next_block)) => {
                block.drawn.append(next.lead);
                block.drawn.append(next_block.drawn);
                block.entries.resize_with(
                    block.entries.len().max(next_block.entries.len()),
                    Line::default,
                );
                for (entry, next) in block.entries.iter_mut().zip(next_block.entries) {
                    entry.append(next);
                }
            }
        }
    }

    fn lines(self) -> Vec<Line<C>> {
        match self.block {
            None => vec![self.lead],
            Some(mut block) => {
                let mut lead = self.lead;
                lead.append(std::mem::take(&mut block.entries[block.attach]));
                block.entries[block.attach] = lead;
                block
                    .entries
                    .into_iter()
                    .rev()
                    .chain([block.drawn])
                    .collect()
            }
        }
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
                lead: Line::stop(stop),
                block: None,
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
    /// block: its stop begins the topmost entry line, which it shares with
    /// directly enclosed blocks. Single-line contents stay on their line.
    fn enclosed(mut self, stop: Stop<C>) -> Self {
        let multiline = self.rows.len() > 1;
        match self.rows.first_mut() {
            None => Self::stop(stop),
            Some(row) => {
                let mut entry = Line::stop(stop);
                let lead = std::mem::take(&mut row.lead);
                match row.block.take() {
                    None if !multiline => {
                        entry.append(lead);
                        row.lead = entry;
                    }
                    None => {
                        row.block = Some(Block {
                            drawn: lead,
                            entries: vec![entry],
                            attach: 0,
                        })
                    }
                    Some(mut block) => {
                        // A label stays beside the first block; the enclosing
                        // stop precedes everything, even a deeper later block.
                        if lead.first.is_some() {
                            let first = &mut block.entries[block.attach];
                            let mut joined = lead;
                            joined.append(std::mem::take(first));
                            *first = joined;
                            block.entries.push(Line::default());
                        }
                        block.attach = block.entries.len() - 1;
                        let top = &mut block.entries[block.attach];
                        entry.append(std::mem::take(top));
                        *top = entry;
                        row.block = Some(block);
                    }
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
