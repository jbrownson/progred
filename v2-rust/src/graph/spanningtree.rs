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

    pub fn get_root(&self, root: &RootSlot) -> Option<&TreeNode> {
        self.roots.get(root)
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

    fn set_collapsed_at_edges(&self, edges: &[Id], collapsed: bool) -> Self {
        if edges.is_empty() {
            Self {
                collapsed: Some(collapsed),
                children: self.children.clone(),
            }
        } else {
            let head = &edges[0];
            let tail = &edges[1..];
            let child_tree = self
                .children
                .get(head)
                .cloned()
                .unwrap_or_else(TreeNode::empty);
            let new_child = child_tree.set_collapsed_at_edges(tail, collapsed);
            Self {
                collapsed: self.collapsed,
                children: self.children.update(head.clone(), new_child),
            }
        }
    }

    fn is_collapsed_at_edges(&self, edges: &[Id]) -> Option<bool> {
        if edges.is_empty() {
            self.collapsed
        } else {
            let child = self.children.get(&edges[0])?;
            child.is_collapsed_at_edges(&edges[1..])
        }
    }
}

impl Default for SpanningTree {
    fn default() -> Self {
        Self::empty()
    }
}
