//! The reading context: a document followed by an ordered set of
//! libraries. Lookup selects the document's definition, then the first
//! library's; reads, follows and evaluation all go through it.

use crate::display::ResolvedCell;
use crate::library::Libraries;
use crate::location::{Location, Start};
use gid::{CellId, Document, Step, Value};

/// One source's definition of a cell.
#[derive(Clone, Copy)]
enum Contribution<'a> {
    Document(&'a Value),
    Library(CellId, &'a Value),
}

impl<'a> Contribution<'a> {
    fn value(self) -> &'a Value {
        match self {
            Self::Document(value) => value,
            Self::Library(_, value) => value,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub doc: &'a Document,
    pub libraries: &'a Libraries,
}

/// How much work an evaluation may do when nothing else sets its fuel.
pub const DEFAULT_FUEL: usize = 1_024;

impl crate::display::Env for Sources<'_> {
    fn apply_expression_runtime(
        &self,
        function: &grap::RuntimeValue,
        arguments: &[(CellId, grap::RuntimeValue)],
    ) -> grap::RuntimeValue {
        grap::apply_expression(
            function,
            arguments.iter().cloned(),
            self,
            None,
            crate::sources::DEFAULT_FUEL,
        )
        .result
    }
    fn evaluate_runtime(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        _steps: &[Step],
    ) -> grap::RuntimeValue {
        grap::evaluate_runtime_at(
            expression,
            Some(grap::SourceOrigin::Input(vec![])),
            self,
            fuel,
        )
        .result
    }
    fn evaluate_runtime_memo(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        steps: &[Step],
    ) -> grap::RuntimeValue {
        self.evaluate_runtime(expression, fuel, steps)
    }
    fn apply_runtime_scoped(
        &self,
        function: &grap::RuntimeValue,
        arguments: &[(CellId, grap::RuntimeValue)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation {
        grap::apply(
            function,
            arguments.iter().cloned(),
            self,
            scope,
            crate::sources::DEFAULT_FUEL,
        )
    }
    fn apply_runtime_memo(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        fuel: usize,
    ) -> grap::RuntimeValue {
        grap::apply_expression(
            &function.into(),
            arguments.iter().map(|(k, v)| (*k, v.into())),
            self,
            None,
            fuel,
        )
        .result
    }

    fn resolve(&self, cell: CellId) -> Option<ResolvedCell<'_>> {
        Sources::resolve(self, cell)
    }
}

impl<'a> Sources<'a> {
    /// The definition lookup selects: the document's, else the first
    /// library's in load order.
    fn definition(&self, cell: CellId) -> Option<Contribution<'a>> {
        self.doc
            .cells
            .value(cell)
            .map(Contribution::Document)
            .or_else(|| {
                self.libraries
                    .definitions(cell)
                    .next()
                    .map(|(library, value)| Contribution::Library(library, value))
            })
    }

    /// The value of the definition lookup selects.
    pub fn value(&self, cell: CellId) -> Option<&'a Value> {
        self.definition(cell).map(Contribution::value)
    }

    /// The definition lookup selects.
    pub fn resolve(&self, cell: CellId) -> Option<ResolvedCell<'a>> {
        self.definition(cell).map(|contribution| ResolvedCell {
            value: contribution.value(),
            library: match contribution {
                Contribution::Document(_) => None,
                Contribution::Library(library, _) => Some(library),
            },
        })
    }

    pub fn root(&self) -> Option<&'a Value> {
        self.doc.root.as_ref()
    }

    /// The value at `path` from the document's root.
    #[cfg(test)]
    pub fn resolve_path(&self, path: &[Step]) -> Option<&'a Value> {
        self.read_from(Start::Root, path)
    }

    pub fn read(&self, location: &Location) -> Option<&'a Value> {
        self.read_from(location.start, &location.path)
    }

    /// The value at `path` from `start`, following links and descending
    /// through ordinary record and list structure. Writes gate separately
    /// on where the location is stored.
    pub fn read_from(&self, start: Start, path: &[Step]) -> Option<&'a Value> {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::SourceLookup);
        path.iter().try_fold(
            match start {
                Start::Root => self.root(),
                Start::Cell(cell) => self.value(cell),
            }?,
            |value, step| self.step(value, step),
        )
    }

    /// The value `step` reaches from `value`.
    pub fn step(&self, value: &'a Value, step: &Step) -> Option<&'a Value> {
        match step {
            Step::Follow => self.value(value.as_cell()?),
            Step::Key(label) => value.as_record()?.get(label),
            Step::Element(position) => value.as_list()?.get(position),
        }
    }

    pub fn cells(&self) -> Vec<CellId> {
        let mut cells: Vec<_> = self
            .doc
            .cells
            .cells()
            .copied()
            .chain(self.libraries.cell_ids())
            .collect();
        cells.sort_unstable();
        cells.dedup();
        cells
    }

    /// The library is authoritative only when it supplies the value
    /// and the document does not. A bare cell remains writable.
    pub fn external(&self, cell: CellId) -> bool {
        matches!(self.definition(cell), Some(Contribution::Library(..)))
    }
}

impl grap::Host for Sources<'_> {
    fn resolve(&self, cell: CellId) -> Option<Value> {
        self.value(cell).cloned()
    }

    /// Only the libraries have native code.
    fn implementation(&self, cell: CellId) -> Option<grap::ForeignFunction> {
        self.libraries.implementation(cell).cloned()
    }
}

#[cfg(test)]
#[path = "tests/sources.rs"]
mod tests;
