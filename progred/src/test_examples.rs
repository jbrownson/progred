//! Stable single-shape fixtures for rendering regressions and historical
//! canaries, and the example editors that captures, canaries and lesson tests
//! share.

use gid::{CellId, Path, Step, Value};

pub const SPHERES: &str = include_str!("../../examples/fidget.gid");
pub const TORUS: &str = include_str!("../../examples/fidget-torus.gid");
pub const TANGLECUBE: &str = include_str!("../../examples/fidget-tanglecube.gid");
pub const GYROID: &str = include_str!("../../examples/fidget-gyroid.gid");
pub const CUBE: &str = include_str!("../../examples/fidget-cube.gid");

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

/// Computed content owns annotations below the declaration that produced it.
pub fn result_path(path: &[Step]) -> Path {
    path.iter()
        .cloned()
        .chain([Step::Key(
            crate::libraries::presentation::vocabulary::RESULT,
        )])
        .collect()
}

// Preparation feeds the CAM viewport, which returns with-controls directly.
pub fn cam_controls_path(path: &[Step]) -> Path {
    result_path(path)
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

/// The growing-forest lesson as its tutorial page embeds it.
pub fn website_growing_forest_editor() -> (crate::Editor, crate::gid_text::Binders) {
    use crate::libraries::{
        absent, blob, color, control, controls, f64, grap, layout, list, logic, name, number,
        presentation, text,
    };
    let (doc, fields) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../website/public/lessons/growing-forest.gid"
    )))
    .unwrap();
    let mut editor = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            name::ID,
            text::ID,
            blob::ID,
            absent::ID,
            color::ID,
            control::ID,
            number::ID,
            f64::ID,
            grap::ID,
            layout::ID,
            controls::ID,
            presentation::ID,
            logic::ID,
            list::ID,
            crate::libraries::sequence::ID,
        ])
        .unwrap(),
    );
    editor.stack.projection = crate::web_embed::tutorial_slots(
        Some(&format!("{},{}", fields["second"], fields["first"])),
        editor.stack.projection,
        &editor.stack.libraries,
    )
    .unwrap();
    editor.drawn_menu = false;
    (editor, fields)
}
