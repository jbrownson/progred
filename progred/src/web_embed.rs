//! Browser-host observation and opt-in fixed-slot tutorial presentation.

use crate::display::widget::view::Area;
use crate::location::{Location, Start};
use crate::{Editor, selection::Stage};
use ::grap::path;
use gid::{Document, Resolution, Step, Value};
use std::rc::Rc;

pub(crate) fn libraries(ids: Option<&str>) -> Result<crate::stack::Stack, String> {
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
pub(crate) fn peeled(
    libraries: Option<&str>,
    projections: &str,
    slots: Option<&str>,
    names: bool,
) -> Result<crate::stack::Stack, String> {
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

/// The content height of the document's whole area, `document`, in CSS
/// pixels from the top of the editor. The first frame is built before winit has measured the canvas, in
/// an empty window, so its layout says nothing and reports none.
pub(crate) fn content_height(
    regions: &[crate::display::widget::frame::ViewRegion],
    document: &Area,
    scale: f64,
) -> Option<f64> {
    regions.iter().find_map(|region| {
        let content = region.content?;
        (region.area == *document && region.rect.area() > 0.0)
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
    fn caption(self, libraries: &crate::library::Libraries, same: bool) -> String {
        let drawn = if same {
            "The same value, drawn"
        } else {
            "Drawn"
        };
        match self {
            Level::Full => {
                let names = crate::libraries::name::library_names(libraries)
                    .map(|(_, name)| name)
                    .collect::<Vec<_>>();
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
                "{drawn} with the name, text, blob, and absent libraries, plus numbers but not their arithmetic"
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
    projection: crate::projection::Projection,
    libraries: &crate::library::Libraries,
) -> Result<crate::projection::Projection, String> {
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
                    crate::stack::load_selected(&crate::stack::PLAIN)
                        .map(|stack| stack.projection.partial().clone())
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
            // A slot is a pane of its own, so with the presentation library
            // loaded it presents a render or declaration there, as a pane does.
            let full_entry = libraries
                .iter()
                .any(|(id, _)| id == crate::libraries::presentation::ID)
                .then(|| {
                    crate::display::compose_partials([
                        crate::display::runtime_partial(crate::libraries::presentation::pane_entry),
                        projection.partial().clone(),
                    ])
                });
            let captioned = slots.windows(2).any(|pair| pair[0].1 != pair[1].1);
            let captions = [false, true].map(|same| {
                [Level::Full, Level::Plain, Level::Raw].map(|level| level.caption(libraries, same))
            });
            Ok(projection.with_entry(crate::display::partial(move |input| {
                matches!(input.value, Some(Value::Record(_))).then(|| {
                    let same = same_as_above(&slots, input.value);
                    crate::display::col(
                        0,
                        16.0,
                        slots.iter().zip(same).map(|((key, level), same)| {
                            let view = match level {
                                Level::Full => crate::display::descend(
                                    [Step::Key(*key)],
                                    full_entry.clone(),
                                    Some(full_inside.clone()),
                                ),
                                Level::Plain => crate::display::descend(
                                    [Step::Key(*key)],
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
                                        crate::display::leaf(puri::draw::Leaf::Text {
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
                    )
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
    crate::libraries::name::shallow_cell_with(input, |name| {
        crate::display::selectable_bracket(crate::display::Delim::Paren, name)
    })
}

#[derive(PartialEq, Eq)]
struct Selection {
    /// Whether it's in the document's view rather than one the editor
    /// opened.
    document: bool,
    path: crate::occurrence::Path,
    /// Where the selected data is stored.
    source: Option<Location<'static>>,
    stage: Stage,
}

/// A stored location as JSON: the cell whose definition it starts in, and the
/// library defining that cell when the document doesn't, then the path from
/// there.
fn source(location: &Location) -> serde_json::Value {
    match location.start {
        Start::Root => serde_json::json!({"path": path::value(&location.path)}),
        Start::Cell { cell, resolution } => serde_json::json!({
            "cell": cell,
            "library": match resolution {
                Resolution::Document => None,
                Resolution::Library(library) => Some(library),
            },
            "path": path::value(&location.path),
        }),
    }
}

#[derive(Default)]
struct Changes {
    previous: Option<(Rc<Document>, Option<Selection>)>,
}

impl Changes {
    fn sample(&mut self, editor: &Editor) -> Option<String> {
        let selection = editor.model.selection.as_ref().map(|selected| Selection {
            document: *selected.view() == editor.model.workspace.document,
            path: selected.path().to_vec(),
            source: selected
                .location()
                .and_then(|location| location.stored(&editor.sources()).map(Location::into_owned)),
            stage: selected.stage(&editor.sources()),
        });
        if self
            .previous
            .as_ref()
            .is_some_and(|(doc, prior)| Rc::ptr_eq(doc, &editor.model.doc) && *prior == selection)
        {
            None
        } else {
            let sources = editor.sources();
            let value = editor
                .model
                .selection
                .as_ref()
                .and_then(|selected| selected.value(&sources));
            let state = serde_json::json!({
                "document": &*editor.model.doc,
                "selection": selection.as_ref().map(|selected| serde_json::json!({
                    "view": if selected.document { "document" } else { "opened" },
                    "path": crate::occurrence::value(&selected.path),
                    "source": selected.source.as_ref().map(source),
                    "stage": match selected.stage {
                        Stage::Edge => "value",
                        Stage::Pending => "pending",
                        Stage::Label => "label",
                    },
                    "value": value,
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
#[path = "tests/web_embed/mod.rs"]
mod tests;
