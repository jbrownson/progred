use crate::d::D;
use crate::path::Path;
use crate::selection::Selection;

pub struct DescendNode {
    pub path: Path,
    pub selection: Selection,
    pub is_placeholder: bool,
    pub children: Vec<DescendNode>,
}

pub fn collect_descends(d: &D) -> Vec<DescendNode> {
    match d {
        D::Block(children) | D::Line(children) =>
            children.iter().flat_map(collect_descends).collect(),
        D::Indent(child) | D::NodeHeader { child } | D::FieldLabel { child, .. } =>
            collect_descends(child),
        D::Descend { path, selection, child } => vec![DescendNode {
            path: path.clone(),
            selection: selection.clone(),
            is_placeholder: matches!(child.as_ref(), D::Placeholder { .. }),
            children: collect_descends(child),
        }],
        D::VerticalList { elements, .. } | D::HorizontalList { elements, .. } =>
            elements.iter().flat_map(collect_descends).collect(),
        D::Text(..) | D::Identicon(_) | D::CollapseToggle { .. }
        | D::StringEditor { .. } | D::NumberEditor { .. } | D::Placeholder { .. } => vec![],
    }
}

struct NavContext<'a> {
    node: &'a DescendNode,
    before: &'a [DescendNode],
    after: &'a [DescendNode],
    parent_selection: Option<&'a Selection>,
}

fn find_context<'a>(
    nodes: &'a [DescendNode],
    target: &Path,
    parent_selection: Option<&'a Selection>,
) -> Option<NavContext<'a>> {
    nodes.iter().enumerate().find_map(|(i, node)| {
        if &node.path == target {
            let (before, rest) = nodes.split_at(i);
            Some(NavContext { node, before, after: &rest[1..], parent_selection })
        } else {
            find_context(&node.children, target, Some(&node.selection))
        }
    })
}

fn collect_tab_stops(nodes: &[DescendNode], acc: &mut Vec<Selection>) {
    for node in nodes {
        if node.is_placeholder {
            acc.push(node.selection.clone());
        }
        collect_tab_stops(&node.children, acc);
    }
}

fn tab_stops(nodes: &[DescendNode]) -> Vec<Selection> {
    let mut stops = Vec::new();
    collect_tab_stops(nodes, &mut stops);
    stops
}

pub fn first_placeholder(nodes: &[DescendNode]) -> Option<Selection> {
    tab_stops(nodes).into_iter().next()
}

pub fn arrow_down(nodes: &[DescendNode], current: &Path) -> Option<Selection> {
    find_context(nodes, current, None)
        .and_then(|ctx| ctx.node.children.first())
        .map(|child| child.selection.clone())
}

pub fn arrow_up(nodes: &[DescendNode], current: &Path) -> Option<Selection> {
    find_context(nodes, current, None)
        .and_then(|ctx| ctx.parent_selection)
        .cloned()
}

pub fn arrow_left(nodes: &[DescendNode], current: &Path) -> Option<Selection> {
    find_context(nodes, current, None)
        .and_then(|ctx| ctx.before.last())
        .map(|node| node.selection.clone())
}

pub fn arrow_right(nodes: &[DescendNode], current: &Path) -> Option<Selection> {
    find_context(nodes, current, None)
        .and_then(|ctx| ctx.after.first())
        .map(|node| node.selection.clone())
}

pub fn first_placeholder_from(nodes: &[DescendNode], from: &Path) -> Option<Selection> {
    let stops = tab_stops(nodes);
    let start = stops.iter().position(|s| s.path() == Some(from)).unwrap_or(0);
    stops.into_iter().nth(start)
}

pub fn post_delete(nodes: &[DescendNode], current: &Path) -> Option<Selection> {
    find_context(nodes, current, None).and_then(|ctx|
        ctx.after.first()
            .or_else(|| ctx.before.last())
            .map(|n| n.selection.clone())
            .or_else(|| ctx.parent_selection.cloned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::Path;

    fn node(path: Path, is_placeholder: bool, children: Vec<DescendNode>) -> DescendNode {
        DescendNode {
            selection: Selection::edge(path.clone()),
            path,
            is_placeholder,
            children,
        }
    }

    fn id(n: u64) -> progred_graph::Id {
        progred_graph::Id::Number(ordered_float::OrderedFloat(n as f64))
    }

    fn path_with(labels: &[u64]) -> Path {
        labels.iter().fold(Path::root(), |p, &n| p.child(id(n)))
    }

    #[test]
    fn first_placeholder_finds_depth_first() {
        let tree = vec![
            node(path_with(&[1]), false, vec![
                node(path_with(&[1, 2]), true, vec![]),
                node(path_with(&[1, 3]), true, vec![]),
            ]),
        ];
        assert_eq!(
            first_placeholder(&tree),
            Some(Selection::edge(path_with(&[1, 2])))
        );
    }

    #[test]
    fn first_placeholder_empty_tree() {
        assert_eq!(first_placeholder(&[]), None);
    }

    #[test]
    fn arrow_down_selects_first_child() {
        let tree = vec![
            node(path_with(&[1]), false, vec![
                node(path_with(&[1, 2]), false, vec![]),
                node(path_with(&[1, 3]), false, vec![]),
            ]),
        ];
        assert_eq!(
            arrow_down(&tree, &path_with(&[1])),
            Some(Selection::edge(path_with(&[1, 2])))
        );
    }

    #[test]
    fn arrow_up_selects_parent() {
        let tree = vec![
            node(path_with(&[1]), false, vec![
                node(path_with(&[1, 2]), false, vec![]),
            ]),
        ];
        assert_eq!(
            arrow_up(&tree, &path_with(&[1, 2])),
            Some(Selection::edge(path_with(&[1])))
        );
    }

    #[test]
    fn arrow_up_from_root_returns_none() {
        let tree = vec![node(path_with(&[1]), false, vec![])];
        assert_eq!(arrow_up(&tree, &path_with(&[1])), None);
    }

    #[test]
    fn arrow_left_right_siblings() {
        let tree = vec![
            node(path_with(&[1]), false, vec![
                node(path_with(&[1, 1]), false, vec![]),
                node(path_with(&[1, 2]), false, vec![]),
                node(path_with(&[1, 3]), false, vec![]),
            ]),
        ];
        assert_eq!(
            arrow_right(&tree, &path_with(&[1, 1])),
            Some(Selection::edge(path_with(&[1, 2])))
        );
        assert_eq!(
            arrow_left(&tree, &path_with(&[1, 3])),
            Some(Selection::edge(path_with(&[1, 2])))
        );
        assert_eq!(arrow_left(&tree, &path_with(&[1, 1])), None);
        assert_eq!(arrow_right(&tree, &path_with(&[1, 3])), None);
    }

    #[test]
    fn post_delete_prefers_next_sibling() {
        let tree = vec![
            node(path_with(&[1]), false, vec![
                node(path_with(&[1, 1]), false, vec![]),
                node(path_with(&[1, 2]), false, vec![]),
                node(path_with(&[1, 3]), false, vec![]),
            ]),
        ];
        assert_eq!(
            post_delete(&tree, &path_with(&[1, 2])),
            Some(Selection::edge(path_with(&[1, 3])))
        );
    }

    #[test]
    fn post_delete_falls_back_to_prev_sibling() {
        let tree = vec![
            node(path_with(&[1]), false, vec![
                node(path_with(&[1, 1]), false, vec![]),
                node(path_with(&[1, 2]), false, vec![]),
            ]),
        ];
        assert_eq!(
            post_delete(&tree, &path_with(&[1, 2])),
            Some(Selection::edge(path_with(&[1, 1])))
        );
    }

    #[test]
    fn post_delete_falls_back_to_parent() {
        let tree = vec![
            node(path_with(&[1]), false, vec![
                node(path_with(&[1, 1]), false, vec![]),
            ]),
        ];
        assert_eq!(
            post_delete(&tree, &path_with(&[1, 1])),
            Some(Selection::edge(path_with(&[1])))
        );
    }
}
