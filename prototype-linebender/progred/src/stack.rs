//! The editor's loaded cells, foreign functions, and projection.

use crate::library::{self, Library};
use crate::projection::Projection;
use gid::Cells;

pub struct Stack<World> {
    pub library: Cells,
    pub foreign: grap::ForeignFunctions,
    pub projection: Projection<World>,
}

impl<World> Clone for Stack<World> {
    fn clone(&self) -> Self {
        Self {
            library: self.library.clone(),
            foreign: self.foreign.clone(),
            projection: self.projection.clone(),
        }
    }
}

pub fn load<World>() -> Stack<World> {
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

fn libraries<World>() -> impl Iterator<Item = Library<World>> {
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
