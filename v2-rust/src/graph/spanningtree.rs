use super::id::Id;
use super::path::Path;
use im::HashMap;

#[derive(Debug, Clone)]
pub struct SpanningTree {
    pub roots: HashMap<Id, TreeNode>,
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
            .get(&path.root_slot)
            .cloned()
            .unwrap_or_else(TreeNode::empty);
        let new_root_tree = root_tree.set_collapsed_at_edges(&path.edges, collapsed);
        Self {
            roots: self.roots.update(path.root_slot.clone(), new_root_tree),
        }
    }

    pub fn get_root(&self, root_slot: &Id) -> Option<&TreeNode> {
        self.roots.get(root_slot)
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
}

impl Default for SpanningTree {
    fn default() -> Self {
        Self::empty()
    }
}
