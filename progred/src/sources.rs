//! The reading context: a document followed by an ordered set of
//! libraries. Ordinary lookup selects the document, then the first
//! library definition. A stored Follow step still names its source.

use crate::display::ResolvedCell;
use crate::library::Libraries;
use crate::location::{Location, Start};
use gid::{CellId, Document, Resolution, Step, Value};

/// One source's definition of a cell.
#[derive(Clone, Copy)]
enum Contribution<'a> {
    Document(&'a Value),
    Library(&'a grap::Definition),
}

impl<'a> Contribution<'a> {
    fn value(self) -> &'a Value {
        match self {
            Self::Document(value) => value,
            Self::Library(definition) => definition.value(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub doc: &'a Document,
    pub libraries: &'a Libraries,
}

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
            grap::DEFAULT_FUEL,
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
            grap::DEFAULT_FUEL,
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

    fn apply_scoped(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation<gid::Value> {
        grap::apply_value(
            function,
            arguments.iter().cloned(),
            self,
            scope,
            grap::DEFAULT_FUEL,
        )
    }

    fn evaluate(&self, expression: &Value) -> Value {
        self.evaluate_with_fuel(expression, grap::DEFAULT_FUEL)
    }

    fn evaluate_with_fuel(&self, expression: &Value, fuel: usize) -> Value {
        grap::evaluate_value(expression, self, fuel).result
    }

    fn apply_memo(&self, function: &Value, arguments: &[(CellId, Value)], fuel: usize) -> Value {
        grap::apply_value(function, arguments.iter().cloned(), self, None, fuel).result
    }

    fn resolve(&self, cell: CellId) -> Option<ResolvedCell<'_>> {
        Sources::resolve(self, cell)
    }
}

impl<'a> Sources<'a> {
    /// Every definition of `cell` in lookup order: the document's, then each
    /// library's in load order. Ordinary lookup takes the first.
    fn definitions(&self, cell: CellId) -> impl Iterator<Item = (Resolution, Contribution<'a>)> {
        let libraries = self.libraries;
        self.doc
            .cells
            .value(cell)
            .map(|value| (Resolution::Document, Contribution::Document(value)))
            .into_iter()
            .chain(libraries.definitions(cell).map(|(library, definition)| {
                (
                    Resolution::Library(library),
                    Contribution::Library(definition),
                )
            }))
    }

    pub fn value(&self, cell: CellId, resolution: &Resolution) -> Option<&'a Value> {
        self.values(cell)
            .find(|value| &value.resolution == resolution)
            .map(|value| value.value)
    }

    pub fn values(&self, cell: CellId) -> impl Iterator<Item = ResolvedCell<'a>> {
        self.definitions(cell)
            .map(|(resolution, contribution)| ResolvedCell {
                resolution,
                value: contribution.value(),
                native: matches!(
                    contribution,
                    Contribution::Library(grap::Definition::Foreign(_))
                ),
            })
    }

    /// The definition ordinary lookup selects.
    pub fn resolve(&self, cell: CellId) -> Option<ResolvedCell<'a>> {
        self.values(cell).next()
    }

    pub fn root(&self) -> Option<&'a Value> {
        self.doc.root.as_ref()
    }

    /// The value at `path` from the document's root.
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
                Start::Cell { cell, resolution } => self.value(cell, &resolution),
            }?,
            |value, step| match step {
                Step::Follow(resolution) => self.value(value.as_cell()?, resolution),
                Step::Key(label) => value.as_record()?.get(label),
                Step::Element(position) => value.as_list()?.get(position),
            },
        )
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
        matches!(
            self.definitions(cell).next(),
            Some((Resolution::Library(_), _))
        )
    }
}

impl grap::Host for Sources<'_> {
    fn resolve(&self, cell: CellId) -> Option<(Resolution, grap::Definition)> {
        self.definitions(cell).next().map(|(source, contribution)| {
            (
                source,
                match contribution {
                    Contribution::Document(value) => grap::Definition::Value(value.clone()),
                    Contribution::Library(definition) => definition.clone(),
                },
            )
        })
    }
}

#[cfg(test)]
mod tests;
