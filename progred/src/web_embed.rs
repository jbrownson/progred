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
    /// explain themselves without the surrounding page.
    fn caption(self) -> &'static str {
        match self {
            Level::Full => "Drawn by every library",
            Level::Plain => "Drawn by names, text, and numbers only",
            Level::Raw => "The base projection: no libraries",
        }
    }
}

pub(crate) fn tutorial_slots(
    ids: Option<&str>,
    projection: crate::projection::Projection<Editor>,
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
            let captioned = slots.iter().any(|(_, level)| *level != Level::Full);
            Ok(projection.with_entry(crate::display::partial(move |input| {
                matches!(input.value, Some(Value::Record(_))).then(|| {
                    crate::display::projection::group(crate::display::col(
                        0,
                        16.0,
                        slots.iter().map(|(key, level)| {
                            let view = match level {
                                Level::Full => crate::display::descend(Step::Key(*key), None, None),
                                Level::Plain => crate::display::descend(
                                    Step::Key(*key),
                                    plain.clone(),
                                    plain.clone(),
                                ),
                                Level::Raw => crate::display::descend_raw(Step::Key(*key)),
                            };
                            if captioned {
                                crate::display::col(
                                    1,
                                    4.0,
                                    [
                                        crate::display::leaf(puri::Leaf::Text {
                                            text: level.caption().into(),
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
            assert!(tutorial_slots(Some(ids), Default::default()).is_err());
        }
        assert!(tutorial_slots(None, Default::default()).is_ok());
        for ids in [
            "9940ece27410c72a5308a544890ccc71",
            "9940ece27410c72a5308a544890ccc71,f717b766d250a7b86c5eb842885c4417:plain,5e716c07490849f072b4e9017dd6230d:raw",
        ] {
            assert!(tutorial_slots(Some(ids), Default::default()).is_ok());
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
