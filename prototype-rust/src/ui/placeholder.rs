use progred_core::d::PlaceholderCommit;
use progred_core::editor::Editor;
use progred_core::generated::{name_of, semantics::{ISA, Type, TypeExpression}};
use progred_core::graph::{Gid, Id};
use progred_core::selection::PlaceholderState;
use progred_core::type_possibility::{type_accepts_candidate, type_accepts_isa};
use eframe::egui::{self, Color32, Ui};
use progred_core::ordered_float::OrderedFloat;
use progred_core::path::Path;
use std::collections::HashMap;

const PLACEHOLDER_INPUT_WIDTH: f32 = 150.0;
const PLACEHOLDER_POPUP_MAX_HEIGHT: f32 = 200.0;
const PLACEHOLDER_POPUP_MAX_WIDTH: f32 = 420.0;

pub struct PlaceholderResult {
    pub text_changed: Option<String>,
    pub selection_moved: Option<usize>,
    pub outcome: PlaceholderOutcome,
}

pub enum PlaceholderOutcome {
    Active,
    Commit(PlaceholderCommit),
    Dismiss,
}

#[derive(Clone)]
enum PlaceholderValue {
    Ref(Id),
    Literal(Id),
    NewTyped { isa: Id },
    NewUuid,
}

impl PlaceholderValue {
    fn commit(self) -> PlaceholderCommit {
        match self {
            PlaceholderValue::Ref(id) | PlaceholderValue::Literal(id) => PlaceholderCommit::Existing(id),
            PlaceholderValue::NewTyped { isa } => PlaceholderCommit::NewNode { isa },
            PlaceholderValue::NewUuid => PlaceholderCommit::Existing(Id::new_uuid()),
        }
    }

    fn creates_new_node(&self) -> bool {
        matches!(self, PlaceholderValue::NewTyped { .. } | PlaceholderValue::NewUuid)
    }
}

#[derive(Clone)]
struct PlaceholderEntry {
    value: PlaceholderValue,
    display: String,
    disambiguation: Option<String>,
    magic: bool,
    possible: bool,
}

struct NamedThing {
    name: String,
    id: Id,
}

fn named_things(editor: &Editor) -> Vec<NamedThing> {
    let mut by_id: HashMap<Id, NamedThing> = HashMap::new();
    let lib = editor.lib();

    for uuid in editor.doc.gid.entities() {
        let id = Id::Uuid(*uuid);
        if let Some(name) = name_of(&lib, &id) {
            by_id.insert(id.clone(), NamedThing { name, id });
        }
    }

    for uuid in editor.semantics.gid.entities() {
        let id = Id::Uuid(*uuid);
        if let Some(name) = name_of(&lib, &id) {
            by_id.entry(id.clone()).or_insert(NamedThing { name, id });
        }
    }

    let mut things: Vec<NamedThing> = by_id.into_values().collect();
    things.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    things
}

fn disambiguation(gid: &impl Gid, id: &Id) -> Option<String> {
    let isa = gid.get(id, &ISA.into())?;
    name_of(gid, isa)
}

fn is_type_node(gid: &impl Gid, id: &Id) -> bool {
    Type::try_wrap(gid, id).is_some()
}

fn filter_entries(entries: &[PlaceholderEntry], needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return (0..entries.len()).map(|i| (i, 0)).collect();
    }

    let mut result: Vec<(usize, usize)> = Vec::new();
    let mut accepted: Vec<bool> = vec![false; entries.len()];

    let needle_lower = needle.to_lowercase();
    let display_lowers: Vec<String> = entries.iter().map(|e| e.display.to_lowercase()).collect();

    let tiers: Vec<Box<dyn Fn(usize) -> bool>> = vec![
        Box::new(|i| entries[i].display.starts_with(needle)),
        Box::new(|i| entries[i].display.contains(needle)),
        Box::new(|i| display_lowers[i].starts_with(&needle_lower)),
        Box::new(|i| display_lowers[i].contains(&needle_lower)),
        Box::new(|i| fuzzy_match(&entries[i].display, needle)),
        Box::new(|i| fuzzy_match(&display_lowers[i], &needle_lower)),
    ];

    for (tier, predicate) in tiers.iter().enumerate() {
        for i in 0..entries.len() {
            if !accepted[i] && predicate(i) {
                accepted[i] = true;
                result.push((i, tier + 1));
            }
        }
    }

    result.sort_by(|a, b| {
        a.1.cmp(&b.1).then_with(|| {
            let pct_a = needle.len() as f64 / entries[a.0].display.len().max(1) as f64;
            let pct_b = needle.len() as f64 / entries[b.0].display.len().max(1) as f64;
            pct_b.partial_cmp(&pct_a).unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    result
}

fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    let mut hay_chars = haystack.chars();
    for nc in needle.chars() {
        loop {
            match hay_chars.next() {
                Some(hc) if hc == nc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

fn entry_job(ui: &Ui, entry: &PlaceholderEntry) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let text_color = if entry.possible {
        ui.visuals().text_color()
    } else {
        Color32::from_gray(140)
    };
    job.append(&entry.display, 0.0, egui::TextFormat::simple(
        egui::TextStyle::Body.resolve(ui.style()),
        text_color,
    ));
    if let Some(dis) = &entry.disambiguation {
        job.append(&format!(" ({dis})"), 0.0, egui::TextFormat::simple(
            egui::TextStyle::Body.resolve(ui.style()),
            Color32::from_gray(160),
        ));
    }
    job
}

fn popup_content_height(ui: &Ui, entries: &[PlaceholderEntry]) -> f32 {
    let row_height = ui.spacing().interact_size.y;
    let row_spacing = ui.spacing().item_spacing.y;

    entries.len() as f32 * row_height
        + entries.len().saturating_sub(1) as f32 * row_spacing
}

fn popup_width(ui: &Ui, entries: &[PlaceholderEntry], anchor_width: f32) -> f32 {
    let widest_entry = entries.iter()
        .map(|entry| ui.fonts_mut(|fonts| fonts.layout_job(entry_job(ui, entry))).rect.width())
        .fold(anchor_width, f32::max);
    let button_padding = ui.spacing().button_padding.x * 2.0;
    let scrollbar_width = (popup_content_height(ui, entries) > PLACEHOLDER_POPUP_MAX_HEIGHT)
        .then(|| ui.spacing().scroll.allocated_width())
        .unwrap_or(0.0);

    (widest_entry + button_padding + scrollbar_width)
        .clamp(anchor_width, PLACEHOLDER_POPUP_MAX_WIDTH)
}

fn popup_height(ui: &Ui, entries: &[PlaceholderEntry]) -> f32 {
    popup_content_height(ui, entries).min(PLACEHOLDER_POPUP_MAX_HEIGHT)
}

fn build_entries(editor: &Editor, filter: &str, expected_type: Option<&TypeExpression>) -> Vec<PlaceholderEntry> {
    let things = named_things(editor);
    let lib = editor.lib();

    // Data entries (references to existing named nodes)
    let mut all_entries: Vec<PlaceholderEntry> = things.iter().map(|t| PlaceholderEntry {
        value: PlaceholderValue::Ref(t.id.clone()),
        display: t.name.clone(),
        disambiguation: disambiguation(&lib, &t.id),
        magic: false,
        possible: expected_type.map_or(true, |et| type_accepts_candidate(&lib, &t.id, et).unwrap_or(false)),
    }).collect();

    // "New X" constructor entries for type nodes
    for t in &things {
        if is_type_node(&lib, &t.id) {
            all_entries.push(PlaceholderEntry {
                value: PlaceholderValue::NewTyped { isa: t.id.clone() },
                display: format!("new {}", t.name),
                disambiguation: None,
                magic: false,
                possible: expected_type.map_or(true, |et| type_accepts_isa(&lib, &t.id, et).unwrap_or(false)),
            });
        }
    }

    // Magic entries (string/number literals)
    let trimmed = filter.trim_start_matches('"').trim_end_matches('"');
    if !trimmed.is_empty() {
        all_entries.push(PlaceholderEntry {
            value: PlaceholderValue::Literal(Id::String(trimmed.to_string())),
            display: format!("\"{}\"", trimmed),
            disambiguation: None,
            magic: true,
            possible: expected_type.map_or(true, |et| type_accepts_candidate(&lib, &Id::String(trimmed.to_string()), et).unwrap_or(false)),
        });
    }
    if let Ok(n) = filter.parse::<f64>() {
        all_entries.push(PlaceholderEntry {
            value: PlaceholderValue::Literal(Id::Number(OrderedFloat(n))),
            display: n.to_string(),
            disambiguation: None,
            magic: true,
            possible: expected_type.map_or(true, |et| type_accepts_candidate(&lib, &Id::Number(OrderedFloat(n)), et).unwrap_or(false)),
        });
    }

    // Plain "New node" entry
    all_entries.push(PlaceholderEntry {
        value: PlaceholderValue::NewUuid,
        display: "New node".to_string(),
        disambiguation: None,
        magic: false,
        possible: expected_type.is_none(),
    });

    // Filter
    let filtered_indices = filter_entries(&all_entries, filter);

    // Sort: possible first, then creation before references/literals, then non-magic before magic,
    // then by filter tier.
    let mut sorted: Vec<(usize, usize)> = filtered_indices;
    sorted.sort_by(|a, b| {
        let a_possible = all_entries[a.0].possible;
        let b_possible = all_entries[b.0].possible;
        let a_creates = all_entries[a.0].value.creates_new_node();
        let b_creates = all_entries[b.0].value.creates_new_node();
        let a_magic = all_entries[a.0].magic;
        let b_magic = all_entries[b.0].magic;
        b_possible.cmp(&a_possible)
            .then_with(|| b_creates.cmp(&a_creates))
            .then_with(|| a_magic.cmp(&b_magic))
            .then_with(|| a.1.cmp(&b.1))
    });

    sorted.into_iter().map(|(i, _)| all_entries[i].clone()).collect()
}

pub fn render(ui: &mut Ui, editor: &Editor, ps: &PlaceholderState, expected_type: Option<&TypeExpression>, path: &Path, focus_map: &mut HashMap<egui::Id, Path>) -> PlaceholderResult {
    let entries = build_entries(editor, &ps.text, expected_type);
    let selected_index = if entries.is_empty() { 0 } else { ps.selected_index.min(entries.len() - 1) };

    let mut text = ps.text.clone();
    let text_id = ui.id().with("placeholder_input");
    focus_map.insert(text_id, path.clone());

    let text_response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(text_id)
            .desired_width(PLACEHOLDER_INPUT_WIDTH)
            .hint_text("search...")
    );
    text_response.request_focus();

    let popup_commit = {
        let mut clicked = None;
        let popup_width = popup_width(ui, &entries, text_response.rect.width());
        let popup_height = popup_height(ui, &entries);
        egui::Popup::from_response(&text_response)
            .id(ui.id().with("placeholder_popup"))
            .open(true)
            .width(popup_width)
            .show(|ui| {
                ui.set_width(popup_width);
                ui.set_height(popup_height);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(popup_height)
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                    for (i, entry) in entries.iter().enumerate() {
                        if ui.add(egui::Button::selectable(i == selected_index, entry_job(ui, entry)).truncate()).clicked() {
                            clicked = Some(i);
                        }
                    }
                    });
            });
        clicked
    };

    let text_changed = if ps.text != text {
        Some(text)
    } else {
        None
    };

    let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let arrow_down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
    let arrow_up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));

    let max_index = if entries.is_empty() { 0 } else { entries.len() - 1 };
    let selection_moved = if arrow_down {
        Some((selected_index + 1).min(max_index))
    } else if arrow_up && selected_index > 0 {
        Some(selected_index - 1)
    } else {
        None
    };

    let commit_index = popup_commit.or_else(|| {
        enter.then_some(selected_index)
    });

    let outcome = match commit_index {
        Some(i) if i < entries.len() => {
            PlaceholderOutcome::Commit(entries.into_iter().nth(i).unwrap().value.commit())
        }
        _ if escape || text_response.lost_focus() => PlaceholderOutcome::Dismiss,
        _ => PlaceholderOutcome::Active,
    };

    PlaceholderResult {
        text_changed,
        selection_moved,
        outcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_core::editor::Editor;
    use progred_core::generated::semantics::Type;

    fn displays(entries: &[PlaceholderEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.display.as_str()).collect()
    }

    fn pos(entries: &[PlaceholderEntry], display: &str) -> usize {
        entries.iter().position(|e| e.display == display)
            .unwrap_or_else(|| panic!("entry {:?} not found in {:?}", display, displays(entries)))
    }

    // --- fuzzy_match ---

    #[test]
    fn fuzzy_match_exact() {
        assert!(fuzzy_match("hello", "hello"));
    }

    #[test]
    fn fuzzy_match_subsequence() {
        assert!(fuzzy_match("hello world", "hlo"));
    }

    #[test]
    fn fuzzy_match_no_match() {
        assert!(!fuzzy_match("hello", "xyz"));
    }

    #[test]
    fn fuzzy_match_empty_needle() {
        assert!(fuzzy_match("anything", ""));
    }

    #[test]
    fn fuzzy_match_empty_haystack() {
        assert!(!fuzzy_match("", "a"));
    }

    #[test]
    fn fuzzy_match_order_matters() {
        assert!(!fuzzy_match("world", "dw"));
    }

    // --- filter_entries ---

    fn test_entries(names: &[&str]) -> Vec<PlaceholderEntry> {
        names.iter().map(|n| PlaceholderEntry {
            value: PlaceholderValue::NewUuid,
            display: n.to_string(),
            disambiguation: None,
            magic: false,
            possible: true,
        }).collect()
    }

    fn filtered_displays<'a>(entries: &'a [PlaceholderEntry], needle: &str) -> Vec<&'a str> {
        filter_entries(entries, needle).iter()
            .map(|(i, _)| entries[*i].display.as_str())
            .collect()
    }

    #[test]
    fn filter_empty_needle_returns_all() {
        let entries = test_entries(&["alpha", "beta", "gamma"]);
        assert_eq!(filtered_displays(&entries, ""), vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn filter_exact_prefix_ranks_first() {
        let entries = test_entries(&["person", "a_person", "Person"]);
        let result = filtered_displays(&entries, "person");
        assert_eq!(result[0], "person");
    }

    #[test]
    fn filter_case_insensitive() {
        let entries = test_entries(&["Person", "PERSON", "person"]);
        let result = filtered_displays(&entries, "person");
        assert!(result.contains(&"Person"));
        assert!(result.contains(&"PERSON"));
    }

    #[test]
    fn filter_substring_match() {
        let entries = test_entries(&["my_field", "other"]);
        let result = filtered_displays(&entries, "field");
        assert!(result.contains(&"my_field"));
        assert!(!result.contains(&"other"));
    }

    #[test]
    fn filter_fuzzy_match() {
        let entries = test_entries(&["my_field_name", "xyz"]);
        let result = filtered_displays(&entries, "mfn");
        assert!(result.contains(&"my_field_name"));
        assert!(!result.contains(&"xyz"));
    }

    #[test]
    fn filter_no_matches() {
        let entries = test_entries(&["alpha", "beta"]);
        let result = filtered_displays(&entries, "zzz");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_entry_appears_only_once() {
        // "person" matches tier 1 (exact prefix) — should not also appear in tier 2 (contains)
        let entries = test_entries(&["person", "a_person"]);
        let result = filter_entries(&entries, "person");
        let person_count = result.iter().filter(|(i, _)| entries[*i].display == "person").count();
        assert_eq!(person_count, 1);
    }

    #[test]
    fn filter_shorter_match_ranks_higher_in_same_tier() {
        let entries = test_entries(&["type_expression", "type"]);
        let result = filtered_displays(&entries, "type");
        assert_eq!(result[0], "type");
    }

    // --- build_entries ---

    #[test]
    fn new_entries_sort_before_references() {
        let mut editor = Editor::new();
        Type::new(&mut editor.doc.gid, Some("person"), None);

        let entries = build_entries(&editor, "person", None);
        assert!(pos(&entries, "new person") < pos(&entries, "person"));
    }

    #[test]
    fn string_literal_entry_appears_for_nonempty_filter() {
        let editor = Editor::new();
        let entries = build_entries(&editor, "hello", None);
        assert!(entries.iter().any(|e| e.display == "\"hello\""));
    }

    #[test]
    fn string_literal_strips_quotes() {
        let editor = Editor::new();
        let entries = build_entries(&editor, "\"hello\"", None);
        assert!(entries.iter().any(|e| e.display == "\"hello\""));
    }

    #[test]
    fn number_literal_entry_appears() {
        let editor = Editor::new();
        let entries = build_entries(&editor, "42", None);
        assert!(entries.iter().any(|e| e.display == "42"));
    }

    #[test]
    fn no_literal_for_empty_filter() {
        let editor = Editor::new();
        let entries = build_entries(&editor, "", None);
        assert!(!entries.iter().any(|e| e.magic));
    }

    #[test]
    fn new_node_entry_always_present() {
        let editor = Editor::new();
        let entries = build_entries(&editor, "", None);
        assert!(entries.iter().any(|e| e.display == "New node"));
    }

    #[test]
    fn possible_entries_sort_before_impossible() {
        let mut editor = Editor::new();
        // Create a type with a record body so it's a valid expected type
        use progred_core::generated::semantics::{Record, TypeExpression, NAME};

        let record = Record::new(&mut editor.doc.gid, None);
        let target_type = Type::new(&mut editor.doc.gid, Some("Target"), Some(&TypeExpression::wrap(record.uuid)));

        // Create another type that won't match
        let other_record = Record::new(&mut editor.doc.gid, None);
        let other_type = Type::new(&mut editor.doc.gid, Some("Other"), Some(&TypeExpression::wrap(other_record.uuid)));

        // Create an instance of Target (will be possible)
        let target_uuid = progred_core::graph::Uuid::new_v4();
        editor.doc.gid.set(target_uuid, ISA.into(), target_type.id());
        editor.doc.gid.set(target_uuid, NAME.into(), Id::String("my_target".into()));

        // Create an instance of Other (will be impossible)
        let other_uuid = progred_core::graph::Uuid::new_v4();
        editor.doc.gid.set(other_uuid, ISA.into(), other_type.id());
        editor.doc.gid.set(other_uuid, NAME.into(), Id::String("my_other".into()));

        let expected = TypeExpression::wrap(target_type.uuid);
        let entries = build_entries(&editor, "", Some(&expected));

        let target_pos = pos(&entries, "my_target");
        let other_pos = pos(&entries, "my_other");
        assert!(target_pos < other_pos, "possible entry should sort before impossible");
        assert!(entries[target_pos].possible);
        assert!(!entries[other_pos].possible);
    }

    #[test]
    fn magic_entries_sort_after_non_magic() {
        let mut editor = Editor::new();
        Type::new(&mut editor.doc.gid, Some("something"), None);

        // "something" matches both the type ref and a string literal
        let entries = build_entries(&editor, "something", None);
        let ref_pos = pos(&entries, "something");
        let literal_pos = pos(&entries, "\"something\"");
        assert!(ref_pos < literal_pos, "non-magic should sort before magic");
    }

    #[test]
    fn no_expected_type_all_possible() {
        let mut editor = Editor::new();
        Type::new(&mut editor.doc.gid, Some("anything"), None);

        let entries = build_entries(&editor, "", None);
        // When no expected type, all non-"New node" entries should be possible
        for entry in &entries {
            if entry.display != "New node" {
                assert!(entry.possible, "{:?} should be possible with no expected type", entry.display);
            }
        }
    }

    #[test]
    fn new_entry_only_for_type_nodes() {
        let mut editor = Editor::new();
        use progred_core::generated::semantics::{Record, Field, TypeExpression};

        // Field is a Type node (has ISA=Type) → should get "new Field"
        // But a Field *instance* is not a Type node → should not get "new my_field"
        let record = Record::new(&mut editor.doc.gid, None);
        Type::new(&mut editor.doc.gid, Some("MyType"), Some(&TypeExpression::wrap(record.uuid)));
        Field::new(&mut editor.doc.gid, Some("my_field"), None);

        let entries = build_entries(&editor, "", None);
        let displays = displays(&entries);
        assert!(displays.contains(&"new MyType"), "Type nodes should get new entries");
        assert!(!displays.contains(&"new my_field"), "non-Type nodes should not get new entries");
    }

    #[test]
    fn disambiguation_shows_isa_name() {
        let mut editor = Editor::new();
        use progred_core::generated::semantics::{Record, TypeExpression, NAME};

        let record = Record::new(&mut editor.doc.gid, None);
        let my_type = Type::new(&mut editor.doc.gid, Some("Person"), Some(&TypeExpression::wrap(record.uuid)));

        // Create an instance of Person
        let instance_uuid = progred_core::graph::Uuid::new_v4();
        editor.doc.gid.set(instance_uuid, ISA.into(), my_type.id());
        editor.doc.gid.set(instance_uuid, NAME.into(), Id::String("alice".into()));

        let entries = build_entries(&editor, "alice", None);
        let alice = entries.iter().find(|e| e.display == "alice").unwrap();
        assert_eq!(alice.disambiguation, Some("Person".to_string()));
    }

    #[test]
    fn new_node_impossible_when_type_expected() {
        let mut editor = Editor::new();
        use progred_core::generated::semantics::{Record, TypeExpression};

        let record = Record::new(&mut editor.doc.gid, None);
        let t = Type::new(&mut editor.doc.gid, Some("Foo"), Some(&TypeExpression::wrap(record.uuid)));

        let expected = TypeExpression::wrap(t.uuid);
        let entries = build_entries(&editor, "", Some(&expected));

        let new_node = entries.iter().find(|e| e.display == "New node").unwrap();
        assert!(!new_node.possible, "New node should be impossible when type is expected");
    }
}
