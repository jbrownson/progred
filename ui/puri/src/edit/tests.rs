use super::*;
use crate::draw::{DrawCmd, DrawList, GlyphRun, Shape};
use crate::handler::Handler;
use kurbo::Stroke;
use peniko::ImageData;
use ui_events::keyboard::{KeyState, Modifiers};

struct DrawFrame {
    list: DrawList,
    handler: Handler<()>,
}

impl crate::draw::CanvasSink for DrawFrame {
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        self.list.image(image, transform);
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        self.list.fill(shape, brush, transform);
    }

    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        self.list.stroke(shape, style, brush, transform);
    }

    fn draw_glyphs(&mut self, run: GlyphRun) {
        self.list.glyph_run(run);
    }

    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn crate::draw::CanvasSink) + '_>,
    ) {
        let outer = std::mem::take(&mut self.list.0);
        content(self);
        let children = std::mem::replace(&mut self.list.0, outer);
        self.list.0.push(DrawCmd::Clip {
            shape,
            transform,
            children,
        });
    }
}

impl HasHandler<()> for DrawFrame {
    type Input = ();

    fn handler(&mut self) -> &mut Handler<()> {
        &mut self.handler
    }
}

fn contexts() -> (FontContext, LayoutContext<Brush>) {
    (FontContext::new(), LayoutContext::new())
}

fn state(text: &str) -> LineEditState {
    LineEditState::new(text)
}

fn presentation() -> LineEditPresentation {
    LineEditPresentation::new(16.0, Brush::default())
}

#[derive(Default)]
struct MemoryClipboard(Option<String>);

impl TextClipboard for MemoryClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.0.clone()
    }

    fn set_text(&mut self, text: &str) {
        self.0 = Some(text.to_string());
    }
}

fn key_event(key: Key, modifiers: Modifiers) -> KeyboardEvent {
    KeyboardEvent {
        key,
        modifiers,
        state: KeyState::Down,
        ..Default::default()
    }
}

fn press(
    state: &mut LineEditState,
    fonts: &mut FontContext,
    layouts: &mut LayoutContext<Brush>,
    key: Key,
    modifiers: Modifiers,
) -> bool {
    press_with(&presentation(), state, fonts, layouts, key, modifiers)
}

fn press_with(
    presentation: &LineEditPresentation,
    state: &mut LineEditState,
    fonts: &mut FontContext,
    layouts: &mut LayoutContext<Brush>,
    key: Key,
    modifiers: Modifiers,
) -> bool {
    press_with_clipboard(
        presentation,
        state,
        fonts,
        layouts,
        &mut MemoryClipboard::default(),
        key,
        modifiers,
    )
}

fn press_with_clipboard(
    presentation: &LineEditPresentation,
    state: &mut LineEditState,
    fonts: &mut FontContext,
    layouts: &mut LayoutContext<Brush>,
    clipboard: &mut dyn TextClipboard,
    key: Key,
    modifiers: Modifiers,
) -> bool {
    state.handle_key(
        presentation,
        fonts,
        layouts,
        clipboard,
        if cfg!(target_os = "macos") {
            crate::keyboard::CommandModifier::Meta
        } else {
            crate::keyboard::CommandModifier::Control
        },
        &key_event(key, modifiers),
    )
}

#[test]
fn typing_moving_and_deleting() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("");

    for c in ["h", "i", "!"] {
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character(c.into()),
            Modifiers::empty(),
        ));
    }
    assert!(state.text() == "hi!");

    assert!(press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Backspace),
        Modifiers::empty(),
    ));
    assert!(state.text() == "hi");

    press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    );
    press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("a".into()),
        Modifiers::empty(),
    );
    assert!(state.text() == "hai");

    // Unhandled keys decline so they can fall through the handler.
    assert!(!press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
    ));
}

#[test]
fn arrow_modifiers_use_the_hosts_text_navigation_convention() {
    use crate::keyboard::CommandModifier::{Control, Meta};
    let (mut fonts, mut layouts) = contexts();
    let presentation = LineEditPresentation {
        prefix: "[".into(),
        suffix: "]".into(),
        ..presentation()
    };
    for (command, modifiers, key, start, end) in [
        (Meta, Modifiers::META, NamedKey::ArrowLeft, 13, 0),
        (Meta, Modifiers::META, NamedKey::ArrowRight, 0, 13),
        (Meta, Modifiers::ALT, NamedKey::ArrowLeft, 13, 8),
        (Meta, Modifiers::ALT, NamedKey::ArrowRight, 0, 3),
        (Control, Modifiers::CONTROL, NamedKey::ArrowLeft, 13, 8),
        (Control, Modifiers::CONTROL, NamedKey::ArrowRight, 0, 3),
    ] {
        for shift in [false, true] {
            let mut state = state("one two three");
            state.cursor_to(start);
            let modifiers = modifiers
                | if shift {
                    Modifiers::SHIFT
                } else {
                    Modifiers::empty()
                };
            assert!(state.handle_key(
                &presentation,
                &mut fonts,
                &mut layouts,
                &mut MemoryClipboard::default(),
                command,
                &key_event(Key::Named(key), modifiers),
            ));
            assert_eq!(
                state.selection_offsets(),
                (if shift { start } else { end }, end)
            );
            assert_eq!(state.text(), "one two three");
        }
    }
}

#[test]
fn delete_modifiers_use_the_hosts_text_editing_convention() {
    use crate::keyboard::CommandModifier::{Control, Meta};
    let (mut fonts, mut layouts) = contexts();
    let presentation = LineEditPresentation {
        prefix: "[".into(),
        suffix: "]".into(),
        ..presentation()
    };
    for (command, modifiers, key, caret, text) in [
        (Meta, Modifiers::META, NamedKey::Backspace, 7, " three"),
        (Meta, Modifiers::META, NamedKey::Delete, 4, "one "),
        (Meta, Modifiers::ALT, NamedKey::Backspace, 7, "one  three"),
        (Meta, Modifiers::ALT, NamedKey::Delete, 4, "one  three"),
        (
            Meta,
            Modifiers::empty(),
            NamedKey::Backspace,
            7,
            "one tw three",
        ),
        (
            Control,
            Modifiers::CONTROL,
            NamedKey::Backspace,
            7,
            "one  three",
        ),
        (
            Control,
            Modifiers::CONTROL,
            NamedKey::Delete,
            4,
            "one  three",
        ),
    ] {
        let mut state = state("one two three");
        state.cursor_to(caret);
        assert!(state.handle_key(
            &presentation,
            &mut fonts,
            &mut layouts,
            &mut MemoryClipboard::default(),
            command,
            &key_event(Key::Named(key), modifiers),
        ));
        assert_eq!(state.text(), text);
    }
}

#[test]
fn cursor_to_floors_to_a_char_boundary() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("héllo");
    state.cursor_to(2);
    assert!(press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("z".into()),
        Modifiers::empty(),
    ));
    assert!(state.text() == "hzéllo");
}

#[test]
fn boundary_arrows_decline_from_either_seeded_end() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("abc").with_cursor_at_end();
    // At the end, right declines and left grinds inward.
    assert!(!press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::empty(),
    ));
    assert!(press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    ));
    // Re-seeded at the start, left declines immediately and
    // typing prepends.
    state.cursor_to_start();
    assert!(!press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    ));
    assert!(press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("z".into()),
        Modifiers::empty(),
    ));
    assert_eq!(state.text(), "zabc");
}

#[test]
fn delete_keys_decline_on_an_empty_buffer() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("x").with_cursor_at_end();

    assert!(press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Backspace),
        Modifiers::empty(),
    ));
    assert!(state.text() == "");
    for key in [NamedKey::Backspace, NamedKey::Delete] {
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(key),
            Modifiers::empty(),
        ));
    }
}

#[test]
fn presentation_can_change_without_resetting_interaction_state() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("abc");
    state.cursor_to(1);
    let first = presentation();
    let second = LineEditPresentation::new(28.0, Brush::default());

    assert!(press_with(
        &first,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("x".into()),
        Modifiers::empty(),
    ));
    assert!(press_with(
        &second,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("y".into()),
        Modifiers::empty(),
    ));
    assert_eq!(state.text(), "axybc");
}

#[test]
fn affixes_are_armor_not_content() {
    let (mut fonts, mut layouts) = contexts();
    let action = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    let presentation = presentation().with_affixes("\"", "\"");
    let mut state = state("hi").with_cursor_at_end();

    // Typing lands between the affixes; the text stays bare.
    assert!(press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("!".into()),
        Modifiers::empty(),
    ));
    assert_eq!(state.text(), "hi!");

    // Backspace at content start bites the prefix: swallowed
    // whole — handled, nothing changes.
    assert!(press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Home),
        Modifiers::empty(),
    ));
    assert!(press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Backspace),
        Modifiers::empty(),
    ));
    assert_eq!(state.text(), "hi!");

    // Motion that only wanders into an affix is no motion: the
    // boundary arrow still declines to the caller.
    assert!(!press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    ));

    // Select-all reaches the content alone; typing replaces it
    // and the affixes stand.
    assert!(press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("a".into()),
        action,
    ));
    assert!(press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("x".into()),
        Modifiers::empty(),
    ));
    assert_eq!(state.text(), "x");

    // Delete at content end bites the suffix: swallowed too.
    state.cursor_to_end();
    assert!(press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Delete),
        Modifiers::empty(),
    ));
    assert_eq!(state.text(), "x");

    // Emptied, the delete keys decline — the caller's
    // delete-the-value idiom sees through the affixes.
    assert!(press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Backspace),
        Modifiers::empty(),
    ));
    assert_eq!(state.text(), "");
    assert!(!press_with(
        &presentation,
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Backspace),
        Modifiers::empty(),
    ));
}

#[test]
fn word_delete_stays_interior_or_declines_whole() {
    let (mut fonts, mut layouts) = contexts();
    let word = if cfg!(target_os = "macos") {
        Modifiers::ALT
    } else {
        Modifiers::CONTROL
    };
    // Word-delete whose boundary lands in the interior works.
    let presentation = presentation().with_affixes("\"", "\"");
    let mut words = state("hi there");
    words.anchor = 2;
    words.focus = 2;
    assert!(press_with(
        &presentation,
        &mut words,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Backspace),
        word,
    ));
    assert_eq!(words.text(), " there");
    // KNOWN COARSENESS: leading whitespace lets the word boundary
    // reach through it into the prefix, and the bite declines
    // WHOLE — a swallowed no-op where trimming to the interior
    // was arguable. Parley owns the range; the decline is the
    // affix contract.
    let mut leading = state(" hi");
    leading.anchor = 1;
    leading.focus = 1;
    assert!(press_with(
        &presentation,
        &mut leading,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::Backspace),
        word,
    ));
    assert_eq!(leading.text(), " hi");
}

#[test]
fn command_chords_never_insert_text() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("x").with_cursor_at_end();
    for modifiers in [Modifiers::CONTROL, Modifiers::META] {
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("q".into()),
            modifiers,
        ));
    }
    assert!(state.text() == "x");
}

#[test]
fn clipboard_is_a_supplied_capability() {
    let (mut fonts, mut layouts) = contexts();
    let presentation = presentation();
    let action = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    let mut clipboard = MemoryClipboard::default();
    let mut source = state("hello").with_cursor_at_end();
    assert!(press_with_clipboard(
        &presentation,
        &mut source,
        &mut fonts,
        &mut layouts,
        &mut clipboard,
        Key::Character("a".into()),
        action,
    ));
    assert!(press_with_clipboard(
        &presentation,
        &mut source,
        &mut fonts,
        &mut layouts,
        &mut clipboard,
        Key::Character("c".into()),
        action,
    ));

    let mut target = state("");
    assert!(press_with_clipboard(
        &presentation,
        &mut target,
        &mut fonts,
        &mut layouts,
        &mut clipboard,
        Key::Character("v".into()),
        action,
    ));
    assert_eq!(target.text(), "hello");
}

#[test]
fn paste_leaves_a_structure_to_the_caller() {
    struct Structured;
    impl TextClipboard for Structured {
        fn get_text(&mut self) -> Option<String> {
            Some(r#"{"record":[]}"#.into())
        }

        fn set_text(&mut self, _: &str) {}

        fn holds_structure(&mut self) -> bool {
            true
        }
    }
    let (mut fonts, mut layouts) = contexts();
    let mut target = state("12");
    assert!(!press_with_clipboard(
        &presentation(),
        &mut target,
        &mut fonts,
        &mut layouts,
        &mut Structured,
        Key::Character("v".into()),
        if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        },
    ));
    assert_eq!(target.text(), "12");
}

#[test]
fn recorder_exposes_selection_and_caret_geometry() {
    let (mut fonts, mut layouts) = contexts();
    let mut cache = crate::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let mut state = state("abc");
    state.anchor = 0;
    state.focus = 2;
    let style = EditStyle {
        selection: Brush::default(),
        cursor: Brush::default(),
    };
    let edit = text_edit(
        LineEditDescription {
            state: &state,
            focused: true,
            presentation: presentation(),
            style: &style,
            placeholder: None,
        },
        &mut tcx,
    );
    let metrics = edit.metrics();
    let mut frame = DrawFrame {
        list: DrawList::new(),
        handler: Handler::new(),
    };
    edit.place::<(), DrawFrame>(
        &mut frame,
        Placement::root(Rect::new(
            20.0,
            30.0,
            20.0 + metrics.width,
            30.0 + metrics.ascent + metrics.descent,
        )),
        crate::keyboard::CommandModifier::Meta,
        |_, _| false,
    );
    let fills: Vec<Rect> = frame
        .list
        .0
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Fill {
                shape: Shape::Rect(rect),
                ..
            } => Some(*rect),
            _ => None,
        })
        .collect();

    assert_eq!(fills.len(), 2);
    let selection = fills[0];
    let caret = fills[1];
    assert!(selection.width() > caret.width());
    assert_eq!(caret.width(), 1.5);
    assert!(caret.height() > 0.0);
}

#[test]
fn a_focused_editor_reports_its_caret_for_composition() {
    let (mut fonts, mut layouts) = contexts();
    let mut cache = crate::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let state = state("abc").with_cursor_at_end();
    let style = EditStyle {
        selection: Brush::default(),
        cursor: Brush::default(),
    };
    let [focused, unfocused] = [true, false].map(|focused| {
        text_edit(
            LineEditDescription {
                state: &state,
                focused,
                presentation: presentation(),
                style: &style,
                placeholder: None,
            },
            &mut tcx,
        )
    });
    let metrics = focused.metrics();
    let at = |x: f64, y: f64| {
        Placement::root(Rect::new(
            x,
            y,
            x + metrics.width,
            y + metrics.ascent + metrics.descent,
        ))
    };
    let area = focused.input_area(at(20.0, 30.0)).unwrap();
    assert!(area.width() > 0.0 && area.height() > 0.0);
    assert_eq!(
        focused.input_area(at(25.0, 40.0)),
        Some(area + Vec2::new(5.0, 10.0))
    );
    assert_eq!(unfocused.input_area(at(20.0, 30.0)), None);
    // A preedit without a cursor hides the caret, but composition
    // still has its area.
    let composing = LineEditState::from_parts("ab", 1, 1, Some(("XY".to_string(), None)), None);
    let hidden = text_edit(
        LineEditDescription {
            state: &composing,
            focused: true,
            presentation: presentation(),
            style: &style,
            placeholder: None,
        },
        &mut tcx,
    );
    assert!(hidden.geometry().cursor.is_none());
    assert!(hidden.input_area(at(20.0, 30.0)).is_some());
}

#[test]
fn geometry_exposes_the_composed_text_for_plain_text_drawing() {
    let (mut fonts, mut layouts) = contexts();
    let mut cache = crate::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let state = LineEditState::from_parts("ab", 1, 1, Some(("XY".to_string(), Some((2, 2)))), None);
    let style = EditStyle {
        selection: Brush::default(),
        cursor: Brush::default(),
    };
    let geometry = text_edit(
        LineEditDescription {
            state: &state,
            focused: true,
            presentation: presentation().with_affixes("[", "]"),
            style: &style,
            placeholder: None,
        },
        &mut tcx,
    )
    .geometry();
    assert_eq!(geometry.text, "[aXYb]");
    assert!(geometry.cursor.is_some());
}

#[test]
fn selection_replaces_on_insert() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("abc").with_cursor_at_end();

    press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::SHIFT,
    );
    assert_eq!((state.anchor, state.focus), (3, 2));

    press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("z".into()),
        Modifiers::empty(),
    );
    assert!(state.text() == "abz");
}

#[test]
fn boundary_arrows_decline_so_navigation_can_take_them() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("ab");
    // Caret at 0: Left declines, Right moves.
    assert!(!press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    ));
    assert!(press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::empty(),
    ));
    let mut state = state.with_cursor_at_end();
    assert!(!press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::empty(),
    ));
    assert!(!press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::SHIFT,
    ));
    assert!(press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    ));
}

#[test]
fn cursor_end_is_immediate_and_clicks_still_place() {
    let (mut fonts, mut layouts) = contexts();
    let presentation = presentation();
    let mut seeded = state("abc").with_cursor_at_end();
    press(
        &mut seeded,
        &mut fonts,
        &mut layouts,
        Key::Character("z".into()),
        Modifiers::empty(),
    );
    assert!(seeded.text() == "abcz");

    let mut clicked = state("abc").with_cursor_at_end();
    clicked.pointer_down(
        &presentation,
        &mut fonts,
        &mut layouts,
        1.0,
        LineEditPointerDown {
            point: Point::new(0.0, 5.0),
            shift: false,
            count: 1,
        },
    );
    press(
        &mut clicked,
        &mut fonts,
        &mut layouts,
        Key::Character("z".into()),
        Modifiers::empty(),
    );
    assert!(clicked.text() == "zabc");

    // Landing the caret at the end again is plain data.
    clicked.cursor_to_end();
    press(
        &mut clicked,
        &mut fonts,
        &mut layouts,
        Key::Character("y".into()),
        Modifiers::empty(),
    );
    assert!(clicked.text() == "zabcy");
}

#[test]
fn double_click_drag_extends_by_words() {
    let (mut fonts, mut layouts) = contexts();
    let presentation = presentation();
    let mut state = state("hello world");
    let selected = |state: &LineEditState| {
        let (start, end) = (state.anchor.min(state.focus), state.anchor.max(state.focus));
        state.text()[start..end].to_string()
    };

    state.pointer_down(
        &presentation,
        &mut fonts,
        &mut layouts,
        1.0,
        LineEditPointerDown {
            point: Point::new(2.0, 5.0),
            shift: false,
            count: 2,
        },
    );
    assert_eq!(selected(&state), "hello");

    // Extending keeps the word anchor across transient editors...
    assert!(state.pointer_move(
        &presentation,
        &mut fonts,
        &mut layouts,
        1.0,
        Point::new(10_000.0, 5.0),
    ));
    assert_eq!(selected(&state), "hello world");

    // ...and dragging back re-collapses to the anchor word.
    assert!(state.pointer_move(
        &presentation,
        &mut fonts,
        &mut layouts,
        1.0,
        Point::new(2.0, 5.0),
    ));
    assert_eq!(selected(&state), "hello");
}

#[test]
fn drag_extends_selection_until_released() {
    let (mut fonts, mut layouts) = contexts();
    let presentation = presentation();
    let mut state = state("hello world");

    state.pointer_down(
        &presentation,
        &mut fonts,
        &mut layouts,
        1.0,
        LineEditPointerDown {
            point: Point::new(0.0, 5.0),
            shift: false,
            count: 1,
        },
    );
    assert!(state.pointer_move(
        &presentation,
        &mut fonts,
        &mut layouts,
        1.0,
        Point::new(10_000.0, 5.0),
    ));
    assert_eq!((state.anchor, state.focus), (0, 11));

    assert!(state.pointer_up());
    assert!(!state.pointer_up());
    assert!(!state.pointer_move(
        &presentation,
        &mut fonts,
        &mut layouts,
        1.0,
        Point::new(0.0, 5.0),
    ));
}

#[test]
fn decoded_preedits_are_normalized_like_ime_events() {
    let (mut fonts, mut layouts) = contexts();
    let empty = LineEditState::from_parts("ab", 1, 1, Some((String::new(), None)), None);
    assert!(!empty.is_composing());
    let mut state = LineEditState::from_parts("ab", 1, 1, Some(("XY".into(), Some((9, 9)))), None);
    assert!(state.is_composing());
    state.editor(&presentation(), &mut fonts, &mut layouts, 1.0);
    assert!(state.handle_ime(&ImeEvent::Preedit(String::new(), None)));
    assert!(!state.is_composing());
}

#[test]
fn ime_compose_then_commit() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("");

    state.handle_ime(&ImeEvent::Preedit("ni".into(), Some((2, 2))));
    assert!(state.is_composing());
    // The preedit stays out of the base text.
    assert!(state.text() == "");
    // Keys decline while composing.
    assert!(!press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Character("x".into()),
        Modifiers::empty(),
    ));

    state.handle_ime(&ImeEvent::Preedit("".into(), None));
    assert!(!state.is_composing());
    state.handle_ime(&ImeEvent::Commit("你".into()));
    assert!(state.text() == "你");
}

#[test]
fn composing_over_a_selection_consumes_it() {
    let (mut fonts, mut layouts) = contexts();
    let mut state = state("abc").with_cursor_at_end();
    press(
        &mut state,
        &mut fonts,
        &mut layouts,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::SHIFT,
    );
    state.handle_ime(&ImeEvent::Preedit("n".into(), Some((1, 1))));
    assert!(state.text() == "ab");
    state.handle_ime(&ImeEvent::Commit("ñ".into()));
    assert!(state.text() == "abñ");
}
