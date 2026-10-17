//! Nested lists supply structure; everything else is an uninterpreted leaf.
//! Selection is occurrence-based, so repeated values remain independent.
use super::*;
use crate::libraries::path;
use puri_widgets::range_slider::{self, RangeSlider};
use puri_widgets::tree_slider::{self, Cursor, Intent};
use std::ops::Range;

type Key = Vec<gid::Position>;
pub(crate) type ItemDecoration = Rc<
    dyn Fn(&[gid::Position]) -> Option<widget::Decoration<crate::Editor, crate::frame::Hovered>>,
>;

pub(crate) fn encode(range: Range<usize>) -> Value {
    Value::list([f64::value(range.start as f64), f64::value(range.end as f64)])
}

fn tree(value: &Value, key: Key) -> tree_slider::Tree<Key> {
    use tree_slider::Tree;
    match value.as_list() {
        Some(list) => Tree::group(
            key.clone(),
            list.iter().map(|(position, value)| {
                tree(
                    value,
                    key.iter().cloned().chain([position.clone()]).collect(),
                )
            }),
        ),
        None => Tree::leaf(key),
    }
}

pub(super) fn emitted_tree(node: &tree::Node, key: Key) -> tree_slider::Tree<Key> {
    match &node.children {
        Some(children) => tree_slider::Tree::group(
            key.clone(),
            children.iter().map(|(position, child)| {
                emitted_tree(
                    child,
                    key.iter().cloned().chain([position.clone()]).collect(),
                )
            }),
        ),
        None => tree_slider::Tree::leaf(key),
    }
}

fn key_value(key: &Key) -> Value {
    path::value(
        &key.iter()
            .cloned()
            .map(gid::Step::Element)
            .collect::<Vec<_>>(),
    )
}

fn read_key(value: &Value) -> Option<Key> {
    path::read(value)?
        .into_iter()
        .map(|step| match step {
            gid::Step::Element(position) => Some(position),
            _ => None,
        })
        .collect()
}

fn intent_value(intent: &Intent<Key>) -> Value {
    match intent {
        Intent::All => ALL.into(),
        Intent::Between { first, last } => Value::list([key_value(first), key_value(last)]),
    }
}

fn read_intent(value: &Value) -> Option<Intent<Key>> {
    if value.as_cell() == Some(ALL) {
        Some(Intent::All)
    } else {
        let [(_, first), (_, last)] = value.as_list()?.iter().as_slice() else {
            return None;
        };
        let (first, last) = (read_key(first)?, read_key(last)?);
        (first <= last).then_some(Intent::Between { first, last })
    }
}

fn read_cursor(value: &Value) -> Option<Cursor<Key>> {
    let [(_, item), (_, fraction)] = value.as_list()?.iter().as_slice() else {
        return None;
    };
    Some(Cursor {
        item: read_key(item)?,
        fraction: f64::read(fraction)?,
    })
}

#[derive(Clone)]
pub(crate) struct Selection {
    tree: Rc<tree_slider::Tree<Key>>,
    selected: Rc<tree_slider::Selection<Key>>,
}

impl std::ops::Deref for Selection {
    type Target = tree_slider::Selection<Key>;
    fn deref(&self) -> &Self::Target {
        &self.selected
    }
}

impl Selection {
    pub fn new(items: &Value, state: Option<&Value>) -> Self {
        Self::from_tree(tree(items, Vec::new()), state)
    }

    fn from_tree(tree: tree_slider::Tree<Key>, state: Option<&Value>) -> Self {
        let tree = Rc::new(tree);
        let stored: Vec<_> = state
            .and_then(Value::as_list)
            .map(|list| {
                list.values()
                    .map(|value| read_intent(value).unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default();
        let selected = Rc::new(tree_slider::Selection::new(&tree, &stored));
        Self { tree, selected }
    }

    pub fn select(&self, level: usize, range: Range<usize>) -> Self {
        self.select_with_current(level, range, None)
    }

    fn select_with_current(
        &self,
        level: usize,
        range: Range<usize>,
        current: Option<usize>,
    ) -> Self {
        Self {
            tree: self.tree.clone(),
            selected: Rc::new(
                self.selected
                    .select_with_current(&self.tree, level, range, current),
            ),
        }
    }

    pub fn select_all(&self, level: usize) -> Self {
        Self {
            tree: self.tree.clone(),
            selected: Rc::new(self.selected.select_all(&self.tree, level)),
        }
    }

    pub fn state(&self) -> Value {
        Value::list(self.intent.iter().map(intent_value))
    }

    pub fn widgets(
        &self,
        key: CellId,
        width: f64,
        decorate: Option<ItemDecoration>,
    ) -> impl Iterator<Item = Widget> + '_ {
        self.range_widgets(key, width, None, decorate)
    }

    fn range_widgets(
        &self,
        key: CellId,
        width: f64,
        position: Option<f64>,
        decorate: Option<ItemDecoration>,
    ) -> impl Iterator<Item = Widget> + '_ {
        self.rows.iter().rev().map(move |row| {
            range_widget(
                key,
                row.slider.clone(),
                row.level,
                self.clone(),
                position,
                width,
                decorate
                    .as_ref()
                    .map(|decorate| row.keys().map(|key| decorate(key)).collect()),
            )
        })
    }
}

pub(crate) fn cursor_state(selection: &Selection, position: f64) -> Value {
    Value::record([
        (vocabulary::RANGE, selection.state()),
        (
            vocabulary::POSITION,
            selection
                .selected
                .cursor(&selection.tree, position)
                .map(|cursor| Value::list([key_value(&cursor.item), f64::value(cursor.fraction)]))
                .unwrap_or_else(|| Value::list([])),
        ),
    ])
}

pub(super) fn cursor(
    items: &Value,
    state: Option<&Value>,
    initial: f64,
    key: CellId,
    width: f64,
    decorate: Option<ItemDecoration>,
) -> (Vec<Widget>, Value) {
    cursor_from_tree(
        tree(items, Vec::new()),
        state,
        initial,
        key,
        width,
        decorate,
    )
}

pub(super) fn cursor_from_tree(
    tree: tree_slider::Tree<Key>,
    state: Option<&Value>,
    initial: f64,
    key: CellId,
    width: f64,
    decorate: Option<ItemDecoration>,
) -> (Vec<Widget>, Value) {
    let state = state.and_then(Value::as_record);
    let selection =
        Selection::from_tree(tree, state.and_then(|state| state.get(&vocabulary::RANGE)));
    let position = selection.position(
        state
            .and_then(|state| state.get(&vocabulary::POSITION))
            .and_then(read_cursor)
            .and_then(|cursor| selection.tree.position(&cursor))
            .unwrap_or(initial),
    );
    let mut widgets = Vec::new();
    if let Some(slider) = selection.slider(position) {
        let selection = selection.clone();
        widgets.push(slider_widget_with(
            key,
            slider,
            width,
            selection.ticks(),
            Rc::new(move |position| cursor_state(&selection, position)),
        ));
    }
    widgets.extend(selection.range_widgets(key, width, Some(position), decorate));
    (
        widgets,
        Value::record([
            (vocabulary::RANGE, encode(selection.leaves.clone())),
            (vocabulary::POSITION, f64::value(position)),
        ]),
    )
}

struct RangeDrag {
    root: Root,
    path: gid::Path,
    edits: crate::editing::Scope,
    key: CellId,
    slider: RangeSlider,
    drag: range_slider::Drag,
    current: Option<usize>,
    level: usize,
    selection: Selection,
    position: Option<f64>,
    rect: Rect,
    scale: f64,
}

impl widget::gesture::Gesture for RangeDrag {
    fn advance(&mut self, session: &mut crate::editing::Session<'_>, samples: &[Point]) -> bool {
        if let Some(point) = samples.last() {
            let selected = self
                .slider
                .dragged(self.drag, self.rect, self.scale, *point);
            self.selection = if let Some(current) = self.current {
                self.selection
                    .select_with_current(self.level, selected, Some(current))
            } else {
                self.selection.select(self.level, selected)
            };
            self.position = self.position.map(|p| self.selection.position(p));
            let value = self.position.map_or_else(
                || self.selection.state(),
                |p| cursor_state(&self.selection, p),
            );
            let mut edit = self.edits.open(crate::editing::Access::of(session));
            let state = set_state(edit.annotation(&self.root, &self.path), self.key, value);
            edit.annotate(&self.root, &self.path, state);
        }
        false
    }
}

fn range_widget(
    key: CellId,
    slider: RangeSlider,
    level: usize,
    selection: Selection,
    position: Option<f64>,
    width: f64,
    decorations: Option<Vec<Option<widget::Decoration<crate::Editor, crate::frame::Hovered>>>>,
) -> Widget {
    let current = position.and_then(|position| selection.current_item(level, position));
    Rc::new(move |context| {
        let scale = context.inputs.styles.scale;
        let root = context.inputs.view.clone();
        let path = context.path.to_vec();
        let edits = context.inputs.edits.clone();
        let slider = slider.clone();
        let selection = selection.clone();
        let decorations: Vec<_> = decorations
            .iter()
            .flatten()
            .map(|decorate| decorate.as_ref().map(|decorate| decorate(context)))
            .collect();
        let leaf = widget::leaf(
            Extent {
                width: width.max(0.0) * scale,
                ascent: range_slider::HEIGHT * scale,
                descent: 0.0,
            },
            move |output, placement| {
                output.claim(puri::hover::Probe::occludes(placement));
                let painted = slider.clone();
                let painted_slider = slider.clone();
                output
                    .render(move |canvas, _| painted.draw(canvas, placement.rect, scale, current));
                output.handler().on_pointer_down(move |editor, event| {
                    let point = Point::new(event.state.position.x, event.state.position.y);
                    if !puri::interact::is_primary_contact(event) || !placement.contains(point) {
                        return false;
                    }
                    if event.state.count == 2 {
                        let selection = selection.select_all(level);
                        let value = position.map_or_else(
                            || selection.state(),
                            |p| cursor_state(&selection, selection.position(p)),
                        );
                        let mut editor = edits.open(crate::editing::Access::new(editor));
                        let state = set_state(editor.annotation(&root, &path), key, value);
                        editor.annotate(&root, &path, state);
                    } else {
                        let drag = slider.begin(placement.rect, scale, point);
                        editor.session().start_gesture(
                            Box::new(RangeDrag {
                                root: root.clone(),
                                path: path.clone(),
                                edits: edits.clone(),
                                key,
                                drag,
                                current: match drag {
                                    range_slider::Drag::Span { anchor }
                                        if Some(anchor) == current =>
                                    {
                                        current
                                    }
                                    _ => None,
                                },
                                slider: slider.clone(),
                                level,
                                selection: selection.clone(),
                                position,
                                rect: placement.rect,
                                scale,
                            }),
                            &[point],
                        );
                    }
                    true
                });
                for (item, decorate) in decorations.into_iter().enumerate() {
                    if let Some(decorate) = decorate
                        && let Some(rect) = painted_slider.item_rect(placement.rect, scale, item)
                    {
                        decorate(output, puri::Placement::new(rect, placement.clip_rect));
                    }
                }
            },
        );
        measured::pad((PADDING_X * scale, 0.0).into(), leaf)
    })
}

#[cfg(test)]
mod tests;
