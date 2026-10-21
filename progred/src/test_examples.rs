//! Stable single-shape fixtures for rendering regressions and performance
//! canaries, and the example editors that captures, canaries and lesson tests
//! share.

use gid::{CellId, Step, Value};

pub const SPHERES: &str = include_str!("../../examples/fidget.gid");
pub const TORUS: &str = include_str!("../../examples/fidget-torus.gid");
pub const TANGLECUBE: &str = include_str!("../../examples/fidget-tanglecube.gid");
pub const GYROID: &str = include_str!("../../examples/fidget-gyroid.gid");
pub const CUBE: &str = include_str!("../../examples/fidget-cube.gid");

/// The keyboard-navigation fixture, opened from disk rather than the
/// Examples menu.
pub const NAVIGATION: &str = include_str!("../../examples/navigation.gid");

pub const FIDGET: [(&str, &str); 5] = [
    ("spheres", SPHERES),
    ("torus", TORUS),
    ("tanglecube", TANGLECUBE),
    ("gyroid", GYROID),
    ("cube", CUBE),
];

/// The CAM example with its panes declared, drawing its preview as `mode`.
pub fn cam_editor(mode: CellId) -> crate::Editor {
    // Exercise each public preview using the same document, without adding a
    // renderer-selection control to the production example.
    let source = crate::command::Example::Toolpaths.source().replace(
        &crate::libraries::toolpath::vocabulary::PREVIEW_REFINED.to_string(),
        &mode.to_string(),
    );
    let (doc, _) = crate::gid_text::parse(&source).unwrap();
    let declarations = crate::workspace::declarations(doc.root.as_ref());
    let mut editor = crate::test_editor(doc);
    editor.model.workspace.sync_declared(&declarations);
    editor
}

/// Where a pane declared at `path` starts drawing its entry: under the
/// declaration, through a jump to the entry.
pub fn pane_root(path: &[Step]) -> crate::occurrence::Path {
    let mut root = crate::occurrence::of(path);
    root.push(crate::occurrence::Step::jump(
        crate::location::Location::root(path.to_vec()),
    ));
    root
}

/// Where a presentation at `path` in the document view draws its result:
/// computed, beneath the presentation's result step.
pub fn result_path(path: &[Step]) -> crate::occurrence::Path {
    let mut result = crate::occurrence::of(path);
    result.extend([
        crate::occurrence::Step::Key(crate::libraries::presentation::vocabulary::RESULT),
        crate::occurrence::Step::Computed,
    ]);
    result
}

pub fn cam_position(progress: f64) -> Value {
    use crate::libraries::controls::tree_range;
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let tree = crate::libraries::tree::build(
        &Value::from(names["program_tree"]).into(),
        &sources,
        3_000_000,
    )
    .unwrap();
    let selection = tree_range::Selection::new(tree.items.as_value(), None);
    Value::record([
        (
            names["focus"],
            tree_range::cursor_state(&selection, progress * selection.leaves.end as f64),
        ),
        (names["preview_mode"], names["stock"].into()),
    ])
}

/// The growing forest with all of its code, as tutorial.html's closing
/// exercise embeds it.
pub fn website_growing_forest_editor() -> (crate::Editor, crate::gid_text::Binders) {
    crate::tutorials::embed("tutorial.html", "yours")
}
