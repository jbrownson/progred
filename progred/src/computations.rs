//! The editor owns memo roots; libraries compose typed computations in them.

use crate::{libraries::Libraries, sources::Sources, workspace::Root};
use gid::{Document, Step};
use grap::Host;
use incremental::{Input, Roots, Runtime, Source};
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct Snapshot {
    doc: Rc<Document>,
    libraries: Libraries,
}

pub(crate) struct Computations {
    pub runtime: Runtime,
    snapshot: Input<Snapshot>,
    pub definitions: grap::memo::Definitions<Snapshot>,
    roots: Roots<(Root, Vec<Step>)>,
}

impl Default for Computations {
    fn default() -> Self {
        let runtime = Runtime::default();
        let snapshot = runtime.input(Snapshot {
            doc: Rc::new(Document {
                root: None,
                cells: gid::Cells::new(),
            }),
            libraries: Libraries::default(),
        });
        let definitions = Source::new(snapshot.clone(), |snapshot: &Snapshot, cell| {
            grap::memo::Resolved(Host::resolve(
                &Sources {
                    doc: &snapshot.doc,
                    libraries: &snapshot.libraries,
                },
                *cell,
            ))
        });
        Self {
            runtime,
            snapshot,
            definitions,
            roots: Roots::default(),
        }
    }
}

impl Computations {
    pub fn from_sources(sources: Sources<'_>) -> Self {
        let computations = Self::default();
        computations.begin(Rc::new(sources.doc.clone()), sources.libraries.clone());
        computations
    }

    pub fn begin(&self, doc: Rc<Document>, libraries: Libraries) {
        self.snapshot.set_by(Snapshot { doc, libraries }, |a, b| {
            Rc::ptr_eq(&a.doc, &b.doc) && a.libraries.same_storage(&b.libraries)
        });
        self.roots.begin();
    }

    pub fn at<T: 'static>(&self, view: &Root, path: &[Step], create: impl FnOnce() -> T) -> Rc<T> {
        self.roots.get((view.clone(), path.to_vec()), create)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_in_different_views_have_independent_root_lifetimes() {
        let computations = Computations::default();
        let a = Root::document();
        let b = Root::document();
        let first = computations.at(&a, &[], || computations.runtime.input(1));
        let second = computations.at(&b, &[], || computations.runtime.input(2));
        first.set(3);
        assert_eq!(*second.observed().0, 2);
        assert!(Rc::ptr_eq(
            &first,
            &computations.at(&a, &[], || unreachable!())
        ));
        let weak = Rc::downgrade(&first);
        drop(first);
        drop(computations);
        assert!(weak.upgrade().is_none());
    }
}
