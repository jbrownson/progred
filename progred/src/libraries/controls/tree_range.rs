//! Nested lists supply structure; everything else is an uninterpreted leaf.
//! Selection is occurrence-based, so repeated values remain independent.
use super::*;
use crate::libraries::path;
use puri_widgets::range_slider::{self, RangeSlider};
use puri_widgets::tree_slider::{self, Cursor, Intent};
use std::ops::Range;

type Key = Vec<gid::Position>;

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
        let tree = Rc::new(tree(items, Vec::new()));
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
        Self {
            tree: self.tree.clone(),
            selected: Rc::new(self.selected.select(&self.tree, level, range)),
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

    pub fn widgets(&self, key: CellId, width: f64) -> impl Iterator<Item = Widget> + '_ {
        self.range_widgets(key, width, None)
    }

    fn range_widgets(
        &self,
        key: CellId,
        width: f64,
        position: Option<f64>,
    ) -> impl Iterator<Item = Widget> + '_ {
        self.rows.iter().rev().map(move |row| {
            range_widget(
                key,
                row.slider.clone(),
                row.level,
                self.clone(),
                position,
                width,
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
) -> (Vec<Widget>, Value) {
    let state = state.and_then(Value::as_record);
    let selection = Selection::new(items, state.and_then(|state| state.get(&vocabulary::RANGE)));
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
            Rc::new(move |position| cursor_state(&selection, position)),
        ));
    }
    widgets.extend(selection.range_widgets(key, width, Some(position)));
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
    level: usize,
    selection: Selection,
    position: Option<f64>,
    rect: Rect,
    scale: f64,
}

impl widget::gesture::Gesture<crate::Editor> for RangeDrag {
    fn advance(&mut self, editor: &mut crate::Editor, samples: &[Point]) -> bool {
        if let Some(point) = samples.last() {
            let selected = self
                .slider
                .dragged(self.drag, self.rect, self.scale, *point);
            self.selection = self.selection.select(self.level, selected);
            self.position = self.position.map(|p| self.selection.position(p));
            let value = self.position.map_or_else(
                || self.selection.state(),
                |p| cursor_state(&self.selection, p),
            );
            let mut editor = self.edits.open(crate::editing::Access::new(editor));
            let state = set_state(editor.annotation(&self.root, &self.path), self.key, value);
            editor.annotate(&self.root, &self.path, state);
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
) -> Widget {
    Rc::new(move |context| {
        let scale = context.inputs.styles.scale;
        let root = context.inputs.view.clone();
        let path = context.path.to_vec();
        let edits = context.inputs.edits.clone();
        let slider = slider.clone();
        let selection = selection.clone();
        let leaf = widget::leaf(
            Extent {
                width: width.max(0.0) * scale,
                ascent: range_slider::HEIGHT * scale,
                descent: 0.0,
            },
            move |output, placement| {
                output.claim(puri::hover::Probe::occludes(placement));
                let painted = slider.clone();
                output.render(move |canvas, _| painted.draw(canvas, placement.rect, scale));
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
                        crate::editing::start_gesture(
                            editor,
                            Box::new(RangeDrag {
                                root: root.clone(),
                                path: path.clone(),
                                edits: edits.clone(),
                                key,
                                drag: slider.begin(placement.rect, scale, point),
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
            },
        );
        measured::pad((PADDING_X * scale, PADDING_Y * scale).into(), leaf)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn leaf() -> Value {
        Value::record([])
    }

    #[test]
    fn ranges_follow_occurrences_and_irregular_nested_lists() {
        let tree = Value::list([
            Value::list([Value::list([leaf(), leaf()]), leaf()]),
            Value::list([leaf(), leaf()]),
        ]);
        let all = Selection::new(&tree, None);
        assert_eq!(
            all.rows.iter().map(|r| r.slider.count).collect::<Vec<_>>(),
            [2, 3, 5]
        );
        assert_eq!(all.leaves, 0..5);
        let chosen = all.select(2, 1..2).select(0, 0..1);
        assert_eq!(chosen.leaves, 3..4);
        assert_eq!(Selection::new(&tree, Some(&chosen.state())).leaves, 3..4);
        assert_eq!(
            all.select(2, 0..1).select(1, 0..1).select(0, 1..2).leaves,
            1..2
        );
        assert_eq!(Selection::new(&Value::list([]), None).leaves, 0..0);
    }

    #[test]
    fn state_keeps_all_and_clipped_child_intent_separate() {
        let tree = Value::list([Value::list([leaf(), leaf()]), Value::list([leaf(), leaf()])]);
        let all = Selection::new(&tree, None);
        assert_eq!(all.state(), Value::list([ALL.into(), ALL.into()]));
        let first = all.select(1, 0..1);
        let restored = Selection::new(&tree, Some(&first.state())).select_all(1);
        assert_eq!(restored.leaves, 0..4);
        assert_eq!(restored.state(), all.state());
        let chosen = all.select(0, 1..3);
        let clipped = chosen.select(1, 0..1);
        assert_eq!(clipped.leaves, 1..2);
        assert_eq!(clipped.intent[0], chosen.intent[0]);
        assert_eq!(
            Selection::new(&tree, Some(&clipped.state()))
                .select_all(1)
                .leaves,
            1..3
        );
        let (widgets, _) = cursor(
            &tree,
            Some(&cursor_state(&clipped, 1.5)),
            0.0,
            TREE_CURSOR,
            200.0,
        );
        assert_eq!(widgets.len(), 3);
        assert_eq!(clipped.intent[0], chosen.intent[0]);
    }

    #[test]
    fn stored_paths_track_insertions_and_survive_missing_endpoints() {
        let original = Value::list([leaf(), leaf(), leaf(), leaf()]);
        let positions: Vec<_> = original.as_list().unwrap().keys().cloned().collect();
        let all = Selection::new(&original, None);
        let chosen = all.select(0, 1..3);
        let full_span = all.select(0, 0..4);
        let mut edited = original.as_list().unwrap().clone();
        edited.insert(
            gid::position::between(None, positions.first()).unwrap(),
            leaf(),
        );
        edited.insert(
            gid::position::between(Some(&positions[1]), Some(&positions[2])).unwrap(),
            leaf(),
        );
        edited.insert(
            gid::position::between(positions.last(), None).unwrap(),
            leaf(),
        );
        let edited_value = Value::List(edited.clone());
        assert_eq!(
            Selection::new(&edited_value, Some(&chosen.state())).leaves,
            2..5
        );
        assert_eq!(
            Selection::new(&edited_value, Some(&full_span.state())).leaves,
            1..6
        );
        assert_eq!(
            Selection::new(&edited_value, Some(&all.state())).leaves,
            0..7
        );
        let stored_cursor = cursor_state(&all, 2.25);
        let (_, result) = cursor(&edited_value, Some(&stored_cursor), 0.0, TREE_CURSOR, 200.0);
        assert_eq!(
            result
                .as_record()
                .unwrap()
                .get(&POSITION)
                .and_then(f64::read),
            Some(4.25)
        );
        edited.remove(&positions[1]);
        edited.remove(&positions[2]);
        let survivors = Selection::new(&Value::List(edited), Some(&chosen.state()));
        assert_eq!(survivors.leaves, 2..3);
        assert_eq!(survivors.state(), chosen.state());
    }

    #[test]
    fn missing_ranges_and_hidden_rows_do_not_destroy_state() {
        let tree = Value::list([Value::list([]), leaf(), leaf()]);
        let chosen = Selection::new(&tree, None).select(0, 1..2);
        let position = tree.as_list().unwrap().keys().last().unwrap();
        let edited = Value::List(tree.as_list().unwrap().without(position));
        let stale = Selection::new(&edited, Some(&chosen.state()));
        assert_eq!(stale.leaves, 0..1);
        assert_eq!(stale.state(), chosen.state());
        assert_eq!(Selection::new(&tree, Some(&stale.state())).leaves, 1..2);
        let empty = Selection::new(&Value::list([]), Some(&chosen.state()));
        assert_eq!(empty.state(), chosen.state());
        assert!(empty.rows.is_empty());
        for malformed in [Value::record([]), Value::list([f64::value(5.0)])] {
            assert_eq!(Selection::new(&tree, Some(&malformed)).leaves, 0..2);
        }
    }

    #[test]
    fn cursor_keeps_its_leaf_position_only_inside_the_selected_range() {
        let tree = Value::list([Value::list([leaf(), leaf()]), Value::list([leaf(), leaf()])]);
        let all = Selection::new(&tree, None);
        let first = all.select(1, 0..1);
        let second = all.select(1, 1..2);
        assert_eq!(first.position(1.25), 1.25);
        assert_eq!(second.position(1.25), 2.0);
        let (widgets, value) = cursor(
            &tree,
            Some(&cursor_state(&second, 2.75)),
            0.0,
            TREE_CURSOR,
            200.0,
        );
        assert_eq!(widgets.len(), 3);
        let value = value.as_record().unwrap();
        assert_eq!(value.get(&RANGE), Some(&encode(2..4)));
        assert_eq!(value.get(&POSITION).and_then(f64::read), Some(2.75));
        let (widgets, _) = cursor(&Value::list([]), None, 0.0, TREE_CURSOR, 200.0);
        assert!(widgets.is_empty());
    }
}
