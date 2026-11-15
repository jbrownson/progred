//! Stable single-shape fixtures for rendering regressions and performance
//! canaries, and the example editors that captures, canaries and lesson tests
//! share.

use gid::{CellId, Step, Value};

pub const SPHERES: &str = include_str!("../../../examples/fidget.gid");
pub const TORUS: &str = include_str!("../../../examples/fidget-torus.gid");
pub const TANGLECUBE: &str = include_str!("../../../examples/fidget-tanglecube.gid");
pub const GYROID: &str = include_str!("../../../examples/fidget-gyroid.gid");
pub const CUBE: &str = include_str!("../../../examples/fidget-cube.gid");

/// The keyboard-navigation fixture, opened from disk rather than the
/// Examples menu.
pub const NAVIGATION: &str = include_str!("../../../examples/navigation.gid");

/// The value stored at `path` from the root of `doc`, following cells.
pub fn read<'a>(doc: &'a gid::Document, path: &[Step]) -> &'a Value {
    path.iter()
        .try_fold(
            doc.root.as_ref().expect("a root"),
            |value, step| match step {
                Step::Key(key) => value.as_record()?.get(key),
                Step::Element(position) => value.as_list()?.get(position),
                Step::Follow => doc.cells.value(value.as_cell()?),
            },
        )
        .unwrap_or_else(|| panic!("nothing at {path:?}"))
}

/// Where the `nth` section named `name` is stored among the entries of the
/// notebook at `notebook`, from the root of `doc`: its entry, followed where
/// it's a shared section's cell.
pub fn section(doc: &gid::Document, notebook: &[Step], name: &str, nth: usize) -> Vec<Step> {
    use crate::libraries::notebook::vocabulary::ENTRIES;
    let entries = [notebook, &[Step::Key(ENTRIES)]].concat();
    read(doc, &entries)
        .as_list()
        .expect("a notebook's entries")
        .iter()
        .filter_map(|(position, entry)| {
            let at = [entries.clone(), vec![Step::Element(position.clone())]].concat();
            let at = match entry.as_cell() {
                Some(_) => [at, vec![Step::Follow]].concat(),
                None => at,
            };
            (crate::libraries::name::read(read(doc, &at)) == Some(name)).then_some(at)
        })
        .nth(nth)
        .unwrap_or_else(|| panic!("no section {name:?} ({nth})"))
}

/// Where the body of the section [`section`] finds is stored.
pub fn section_body(doc: &gid::Document, notebook: &[Step], name: &str, nth: usize) -> Vec<Step> {
    [
        section(doc, notebook, name, nth),
        vec![Step::Key(crate::libraries::section::vocabulary::BODY)],
    ]
    .concat()
}

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
    crate::test_editor(doc)
}

/// Where the `index`th of a row's `count` parts is, its list positions
/// spread as a new list's.
pub fn part(index: usize, count: usize) -> gid::Path {
    vec![
        Step::Key(crate::workspace::vocabulary::PARTS),
        Step::Element(gid::position::spread(count).remove(index)),
    ]
}

/// Where a starting point's or example's row keeps its main value: beside
/// the preview, its second part.
pub fn main_part() -> gid::Path {
    part(1, 2)
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
