use super::id::Id;
use super::path::{Path, RootSlot};
use im::HashMap;

#[derive(Debug, Clone)]
pub struct SpanningTree {
    pub roots: HashMap<RootSlot, TreeNode>,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub collapsed: Option<bool>,
    pub children: HashMap<Id, TreeNode>,
}

impl SpanningTree {
    pub fn empty() -> Self {
        Self {
            roots: HashMap::new(),
        }
    }

    pub fn set_collapsed_at_path(&self, path: &Path, collapsed: bool) -> Self {
        let root_tree = self
            .roots
            .get(&path.root)
            .cloned()
            .unwrap_or_else(TreeNode::empty);
        let new_root_tree = root_tree.set_collapsed_at_edges(&path.edges, collapsed);
        Self {
            roots: self.roots.update(path.root.clone(), new_root_tree),
        }
    }

    pub fn is_collapsed(&self, path: &Path) -> Option<bool> {
        let root_tree = self.roots.get(&path.root)?;
        root_tree.is_collapsed_at_edges(&path.edges)
    }
}

impl TreeNode {
    pub fn empty() -> Self {
        Self {
            collapsed: None,
            children: HashMap::new(),
        }
    }

    fn get_at_path(&self, path: &[Id]) -> Option<&TreeNode> {
        match path.split_first() {
            None => Some(self),
            Some((head, tail)) => self.children.get(head)?.get_at_path(tail),
        }
    }

    fn set_collapsed_at_edges(&self, edges: &[Id], collapsed: bool) -> Self {
        match edges.split_first() {
            None => Self {
                collapsed: Some(collapsed),
                children: self.children.clone(),
            },
            Some((head, tail)) => {
                let child = self.children.get(head)
                    .cloned()
                    .unwrap_or_else(TreeNode::empty)
                    .set_collapsed_at_edges(tail, collapsed);
                Self {
                    collapsed: self.collapsed,
                    children: self.children.update(head.clone(), child),
                }
            }
        }
    }

    fn is_collapsed_at_edges(&self, edges: &[Id]) -> Option<bool> {
        self.get_at_path(edges)?.collapsed
    }
}

impl Default for SpanningTree {
    fn default() -> Self {
        Self::empty()
    }
}
