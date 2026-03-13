use crate::path::Path;
use im::HashMap;

#[derive(Debug, Clone, Default)]
pub struct SpanningTree {
    collapsed: HashMap<Path, bool>,
}

impl SpanningTree {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_collapsed(&self, path: &Path) -> Option<bool> {
        self.collapsed.get(path).copied()
    }

    pub fn set_collapsed(&mut self, path: &Path, collapsed: bool) {
        self.collapsed.insert(path.clone(), collapsed);
    }
}
