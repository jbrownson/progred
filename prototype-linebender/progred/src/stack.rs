//! The editor's loaded cells, foreign functions, and projection.

use crate::library::{self, Library};
use crate::projection::Projection;
use progred_graph::Cells;

#[derive(Clone)]
pub struct Stack {
    pub library: Cells,
    pub foreign: grap::ForeignFunctions,
    pub projection: Projection,
}

pub fn load() -> Stack {
    let (library, foreign, partials) = libraries().fold(
        (Cells::new(), grap::ForeignFunctions::default(), Vec::new()),
        |(library, foreign, mut partials), next| {
            partials.extend(next.projection);
            (
                library.merged(next.cells),
                foreign.merge(next.functions),
                partials,
            )
        },
    );
    Stack {
        library,
        foreign,
        projection: Projection::new(partials),
    }
}

fn libraries() -> impl Iterator<Item = Library> {
    [
        library::conventions(),
        library::grap(),
        library::absent(),
        library::control(),
        library::f64(),
        library::geometry(),
    ]
    .into_iter()
}
