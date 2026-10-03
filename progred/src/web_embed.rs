//! Browser-host observation and opt-in fixed-slot tutorial presentation.

use crate::{Editor, libraries::path, selection::Stage, workspace};
use gid::{Document, Path, Step, Value};
use std::rc::Rc;

pub(crate) fn libraries(ids: Option<&str>) -> Result<crate::stack::Stack<Editor>, String> {
    match ids {
        None => Ok(crate::stack::load()),
        Some(ids) => {
            let ids = if ids.trim().is_empty() {
                Ok(Vec::new())
            } else {
                ids.split(',')
                    .map(|id| {
                        id.trim()
                            .parse::<gid::CellId>()
                            .map_err(|error| format!("Invalid library ID {id:?}: {error}"))
                    })
                    .collect::<Result<Vec<_>, _>>()
            }?;
            crate::stack::load_selected(&ids)
        }
    }
}

/// The page's libraries with only some of them drawing, as a tutorial peels
/// projections away. Every library's definitions stay loaded, so keys and
/// functions keep their names; with names off, each slot shows the base
/// projection.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) fn peeled(
    libraries: Option<&str>,
    projections: &str,
    slots: Option<&str>,
    names: bool,
) -> Result<crate::stack::Stack<Editor>, String> {
    let mut stack = self::libraries(libraries)?;
    stack.projection = self::libraries(Some(projections))?.projection;
    let slots = match (names, slots) {
        (true, slots) => slots.map(str::to_owned),
        (false, Some(slots)) => Some(
            slots
                .split(',')
                .map(|slot| format!("{}:raw", slot.split(':').next().unwrap_or(slot)))
                .collect::<Vec<_>>()
                .join(","),
        ),
        (false, None) => return Err("Names can only be turned off in tutorial slots".into()),
    };
    stack.projection = tutorial_slots(slots.as_deref(), stack.projection, &stack.libraries)?;
    Ok(stack)
}

/// The document view's content height in CSS pixels from the top of the
/// editor. The first frame is built before winit has measured the canvas, in
/// an empty window, so its layout says nothing and reports none.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) fn content_height(regions: &[crate::placed::ViewRegion], scale: f64) -> Option<f64> {
    regions.iter().find_map(|region| {
        let content = region.content?;
        (matches!(region.root.target(), workspace::Target::Document) && region.rect.area() > 0.0)
            .then(|| (region.rect.y0 / scale + content.y).ceil())
    })
}

/// How a tutorial slot draws its value: with every loaded library's
/// projections, plainly (names, text, and numbers read as themselves while
/// calls stay records), or as Raw draws it. Several slots can show one shared cell at different levels.
#[derive(Clone, Copy, PartialEq)]
enum Level {
    Full,
    Plain,
    Raw,
}

impl Level {
    /// Shown above each slot when slots differ in level, so the views
    /// explain themselves without the surrounding page. A slot showing the
    /// cell the slot above it shows says so.
    fn caption(self, libraries: &crate::libraries::Libraries, same: bool) -> String {
        let drawn = if same {
            "The same value, drawn"
        } else {
            "Drawn"
        };
        match self {
            Level::Full => {
                let names = libraries.names().collect::<Vec<_>>();
                match names.as_slice() {
                    [] => format!("{drawn} with no libraries"),
                    [name] => format!("{drawn} with the {name} library"),
                    [first, second] => format!("{drawn} with the {first} and {second} libraries"),
                    [rest @ .., last] => {
                        format!("{drawn} with the {}, and {last} libraries", rest.join(", "))
                    }
                }
            }
            Level::Plain => format!(
                "{drawn} with the name, text, and blob libraries, plus f64’s numbers but not its arithmetic"
            ),
            Level::Raw if same => "The same value in the base projection: no libraries".into(),
            Level::Raw => "The base projection: no libraries".into(),
        }
    }
}

/// Whether each slot shows the cell the slot above it shows. Equal inline
/// values are copies, not one value, so only shared cells count.
fn same_as_above(slots: &[(gid::CellId, Level)], root: Option<&Value>) -> Vec<bool> {
    let cell = |key: &gid::CellId| {
        root.and_then(Value::as_record)
            .and_then(|fields| fields.get(key))
            .and_then(Value::as_cell)
    };
    slots
        .iter()
        .enumerate()
        .map(|(index, (key, _))| {
            index > 0 && cell(key).is_some() && cell(key) == cell(&slots[index - 1].0)
        })
        .collect()
}

pub(crate) fn tutorial_slots(
    ids: Option<&str>,
    projection: crate::projection::Projection<Editor>,
    libraries: &crate::libraries::Libraries,
) -> Result<crate::projection::Projection<Editor>, String> {
    match ids {
        None => Ok(projection),
        Some(ids) => {
            let slots = ids
                .split(',')
                .map(|entry| {
                    let (id, level) = match entry.trim().split_once(':') {
                        None => (entry.trim(), Level::Full),
                        Some((id, "plain")) => (id, Level::Plain),
                        Some((id, "raw")) => (id, Level::Raw),
                        Some((_, level)) => {
                            return Err(format!("Unknown tutorial slot level {level:?}"));
                        }
                    };
                    id.parse::<gid::CellId>()
                        .map(|id| (id, level))
                        .map_err(|error| format!("Invalid tutorial slot {id:?}: {error}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if slots
                .iter()
                .map(|(id, _)| *id)
                .collect::<std::collections::HashSet<_>>()
                .len()
                != slots.len()
            {
                return Err("Tutorial slots must be distinct".into());
            }
            let plain = slots
                .iter()
                .any(|(_, level)| matches!(level, Level::Plain))
                .then(|| {
                    // Names, text, and numbers read as themselves; calls,
                    // including arithmetic, stay records.
                    crate::stack::load_selected(&[
                        crate::libraries::name::ID,
                        crate::libraries::text::ID,
                        crate::libraries::blob::ID,
                    ])
                    .map(|stack| {
                        crate::display::compose_partials([
                            stack.projection.partial().clone(),
                            crate::display::runtime_partial(|input| {
                                crate::libraries::f64::convention().display(input)
                            }),
                        ])
                    })
                })
                .transpose()?;
            let plain_inside = plain.clone().map(|plain| {
                crate::display::compose_partials([
                    crate::display::runtime_partial(named_reference),
                    plain,
                ])
            });
            // Whatever the libraries leave undrawn, a named cell still reads as
            // its name rather than inlining its definition.
            let full_inside = crate::display::compose_partials([
                projection.partial().clone(),
                crate::display::runtime_partial(named_reference),
            ]);
            let captioned = slots.windows(2).any(|pair| pair[0].1 != pair[1].1);
            let captions = [false, true].map(|same| {
                [Level::Full, Level::Plain, Level::Raw].map(|level| level.caption(libraries, same))
            });
            Ok(projection.with_entry(crate::display::partial(move |input| {
                matches!(input.value, Some(Value::Record(_))).then(|| {
                    let same = same_as_above(&slots, input.value);
                    crate::display::projection::group(crate::display::col(
                        0,
                        16.0,
                        slots.iter().zip(same).map(|((key, level), same)| {
                            let view = match level {
                                Level::Full => crate::display::descend(
                                    Step::Key(*key),
                                    None,
                                    Some(full_inside.clone()),
                                ),
                                Level::Plain => crate::display::descend(
                                    Step::Key(*key),
                                    plain.clone(),
                                    plain_inside.clone(),
                                ),
                                Level::Raw => crate::display::descend_raw(Step::Key(*key)),
                            };
                            if captioned {
                                crate::display::col(
                                    1,
                                    4.0,
                                    [
                                        crate::display::leaf(puri::Leaf::Text {
                                            text: captions[same as usize][*level as usize].clone(),
                                            paint: crate::display::Paint::Face(
                                                crate::display::Face::Dim,
                                            ),
                                            script: puri::text::Script::Normal,
                                        }),
                                        view,
                                    ],
                                )
                            } else {
                                view
                            }
                        }),
                    ))
                })
            })))
        }
    }
}

/// Inside a slot, a named cell no library draws reads as its name, so a
/// call shows which function it calls instead of inlining the definition.
fn named_reference(
    input: &crate::display::ProjectionInput<
        '_,
        Editor,
        crate::frame::Hovered,
        ::grap::RuntimeValue,
    >,
) -> Option<crate::display::Layout<Editor, crate::frame::Hovered>> {
    crate::libraries::grap::shallow_cell_with(input, |name| {
        crate::display::selectable_bracket(crate::display::Delim::Paren, name)
    })
}

#[derive(PartialEq, Eq)]
struct Selection {
    root: workspace::Root,
    path: Path,
    source_path: Option<Path>,
    stage: Stage,
}

#[derive(Default)]
struct Changes {
    previous: Option<(Rc<Document>, Option<Selection>)>,
}

impl Changes {
    fn sample(&mut self, editor: &Editor) -> Option<String> {
        let selection = editor.model.selection.as_ref().map(|selected| Selection {
            root: selected.root().clone(),
            path: selected.path().to_vec(),
            source_path: selected.source_path().map(|path| path.into_owned()),
            stage: selected.stage(&editor.sources()),
        });
        if self
            .previous
            .as_ref()
            .is_some_and(|(doc, prior)| Rc::ptr_eq(doc, &editor.model.doc) && *prior == selection)
        {
            None
        } else {
            let state = serde_json::json!({
                "document": &*editor.model.doc,
                "selection": selection.as_ref().map(|selected| serde_json::json!({
                    "view": match selected.root.target() {
                        workspace::Target::Document => serde_json::json!("document"),
                        workspace::Target::Pane { path: root } => serde_json::json!({"pane": path::value(root)}),
                    },
                    "path": path::value(&selected.path),
                    "source_path": selected.source_path.as_deref().map(path::value),
                    "stage": match selected.stage {
                        Stage::Edge => "value",
                        Stage::Pending => "pending",
                        Stage::Label => "label",
                    },
                })),
            });
            self.previous = Some((editor.model.doc.clone(), selection));
            Some(state.to_string())
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct Observer {
    callback: web_sys::js_sys::Function,
    changes: Changes,
}

#[cfg(target_arch = "wasm32")]
impl Observer {
    pub(crate) fn new(callback: web_sys::js_sys::Function) -> Self {
        Self {
            callback,
            changes: Changes::default(),
        }
    }

    pub(crate) fn notify(&mut self, editor: &Editor) {
        if let Some(state) = self.changes.sample(editor)
            && let Err(error) = self
                .callback
                .call1(&wasm_bindgen::JsValue::NULL, &state.into())
        {
            web_sys::console::error_1(&error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection;
    use gid::{Step, Value};

    #[test]
    fn content_height_waits_for_a_measured_window() {
        let region = |rect| crate::placed::ViewRegion {
            root: workspace::Root::document(),
            rect,
            maximum: kurbo::Vec2::ZERO,
            content: Some(kurbo::Vec2::new(300.0, 120.5)),
        };
        assert_eq!(content_height(&[region(kurbo::Rect::ZERO)], 2.0), None);
        assert_eq!(
            content_height(&[region(kurbo::Rect::new(0.0, 40.0, 600.0, 400.0))], 2.0),
            Some(141.0)
        );
    }

    #[test]
    fn a_slot_showing_the_cell_above_says_it_is_the_same_value() {
        let [a, b, c, shared, other] = [(); 5].map(|_| gid::new_cell_id());
        let slots = [(a, Level::Full), (b, Level::Raw), (c, Level::Raw)];
        let root = Value::record([
            (a, Value::Cell(shared)),
            (b, Value::Cell(shared)),
            (c, Value::Cell(other)),
        ]);
        assert_eq!(same_as_above(&slots, Some(&root)), [false, true, false]);
        let copies = Value::record([
            (a, crate::libraries::f64::value(1.0)),
            (b, crate::libraries::f64::value(1.0)),
        ]);
        assert_eq!(same_as_above(&slots[..2], Some(&copies)), [false, false]);
        let libraries = crate::libraries::Libraries::default();
        assert_eq!(
            Level::Raw.caption(&libraries, true),
            "The same value in the base projection: no libraries"
        );
        assert!(
            Level::Plain
                .caption(&libraries, true)
                .starts_with("The same value, drawn with the name")
        );
    }

    #[test]
    fn tutorial_slots_require_distinct_valid_ids() {
        for ids in [
            "",
            "first",
            "9940ece27410c72a5308a544890ccc71,",
            "9940ece27410c72a5308a544890ccc71,9940ece27410c72a5308a544890ccc71",
            "9940ece27410c72a5308a544890ccc71,9940ece27410c72a5308a544890ccc71:raw",
            "9940ece27410c72a5308a544890ccc71:fancy",
            "9940ece27410c72a5308a544890ccc71:",
        ] {
            assert!(tutorial_slots(Some(ids), Default::default(), &Default::default()).is_err());
        }
        assert!(tutorial_slots(None, Default::default(), &Default::default()).is_ok());
        for ids in [
            "9940ece27410c72a5308a544890ccc71",
            "9940ece27410c72a5308a544890ccc71,f717b766d250a7b86c5eb842885c4417:plain,5e716c07490849f072b4e9017dd6230d:raw",
        ] {
            assert!(tutorial_slots(Some(ids), Default::default(), &Default::default()).is_ok());
        }
    }

    #[test]
    fn library_option_distinguishes_default_empty_subset_and_invalid_input() {
        let ids = [crate::libraries::name::ID, crate::libraries::text::ID];
        let option = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            libraries(Some(&option))
                .unwrap()
                .libraries
                .iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(libraries(Some("")).unwrap().libraries.iter().count(), 0);
        assert!(libraries(None).unwrap().libraries.iter().count() > ids.len());
        assert!(libraries(Some("text")).is_err());
        assert!(libraries(Some(&format!("{option},"))).is_err());
        assert!(libraries(Some(&gid::new_cell_id().to_string())).is_err());
    }

    #[test]
    fn reports_initial_state_edits_and_undo_but_not_hover_or_paint() {
        let mut editor = crate::test_editor(
            crate::gid_text::parse(include_str!("../../website/public/lessons/values.gid"))
                .unwrap()
                .0,
        );
        let mut changes = Changes::default();
        let initial = changes.sample(&editor).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&initial).unwrap()["selection"],
            serde_json::Value::Null
        );
        editor.pointer = Some(kurbo::Point::new(100.0, 100.0));
        assert!(changes.sample(&editor).is_none());
        let original = editor.model.doc.clone();
        Rc::make_mut(&mut editor.model.doc).root = Some(Value::list([]));
        assert!(changes.sample(&editor).is_some());
        editor.model.doc = original;
        assert_eq!(changes.sample(&editor).unwrap(), initial);
    }

    #[test]
    fn reports_pending_selection_and_committed_value_at_the_same_path() {
        let mut editor = crate::test_editor(
            crate::gid_text::parse(r#"{"root": ["apples", "pears", "plums"]}"#)
                .unwrap()
                .0,
        );
        let mut changes = Changes::default();
        changes.sample(&editor);
        let list = editor.model.doc.root.as_ref().unwrap().as_list().unwrap();
        let positions: Vec<_> = list.keys().cloned().collect();
        let position = gid::position::between(positions.first(), positions.get(1)).unwrap();
        editor.model.selection = Some(selection::Selection::edge(
            &editor.model.workspace.document.root,
            vec![Step::Element(position.clone())],
        ));
        let pending: serde_json::Value =
            serde_json::from_str(&changes.sample(&editor).unwrap()).unwrap();
        assert_eq!(pending["selection"]["stage"], "pending");
        assert_eq!(pending["selection"]["view"], "document");
        let mut items = list.clone();
        items.insert(position, crate::libraries::text::value("peaches"));
        Rc::make_mut(&mut editor.model.doc).root = Some(Value::List(items));
        let committed: serde_json::Value =
            serde_json::from_str(&changes.sample(&editor).unwrap()).unwrap();
        assert_eq!(committed["selection"]["stage"], "value");
        editor.model.selection = Some(selection::Selection::edge(
            &editor.model.workspace.document.root,
            vec![],
        ));
        let whole: serde_json::Value =
            serde_json::from_str(&changes.sample(&editor).unwrap()).unwrap();
        assert_eq!(whole["selection"]["path"], serde_json::json!({"list": []}));
        assert!(changes.sample(&editor).is_none());
    }
}
