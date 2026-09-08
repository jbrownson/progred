use gid::Path;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Document,
    Pane { path: Path },
}

/// A view's transient identity and immutable data root. Equality is
/// allocation identity: two views of the same value remain distinct.
#[derive(Clone)]
pub struct Root(Rc<Target>);

impl Root {
    pub fn document() -> Self {
        Self(Rc::new(Target::Document))
    }

    pub fn pane(path: Path) -> Self {
        Self(Rc::new(Target::Pane { path }))
    }

    pub fn target(&self) -> &Target {
        &self.0
    }
}

impl PartialEq for Root {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Root {}

impl Hash for Root {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}

impl std::fmt::Debug for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Root")
            .field(&(Rc::as_ptr(&self.0) as usize))
            .field(self.target())
            .finish()
    }
}
