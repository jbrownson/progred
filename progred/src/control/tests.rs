use super::computer::vocabulary::*;
use super::*;
use crate::libraries::{f64, text};
use gid::{Cells, Step, Value};
use puri::keyboard::CommandModifier;

fn session(root: Value) -> Rc<RefCell<Session>> {
    Rc::new(RefCell::new(Session::new(
        Document {
            root: Some(root),
            cells: Cells::new(),
        },
        Binders::new(),
    )))
}

/// The root of the reply to `expression`, sent as a request document.
fn send(session: &Rc<RefCell<Session>>, expression: Value) -> Value {
    let binders = computer::binders();
    let request = gid_text::print(
        &Document {
            root: Some(expression),
            cells: Cells::new(),
        },
        &binders,
    );
    let reply = respond(&Session::libraries(session), &binders, request.as_bytes());
    gid_text::parse(&reply).unwrap().0.root.unwrap()
}

fn at(point: Point) -> Value {
    Value::list([f64::value(point.x), f64::value(point.y)])
}

fn call(function: CellId, arguments: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::call(Value::from(function), arguments)
}

/// `key` held with the platform's command modifier, as an agent spells it.
fn command(key: &str) -> Value {
    call(
        KEY,
        [(
            TEXT,
            text::value(match crate::modifiers::native() {
                CommandModifier::Meta => format!("cmd+{key}"),
                CommandModifier::Control => format!("ctrl+{key}"),
            }),
        )],
    )
}

#[test]
fn a_click_selects_what_is_drawn_there() {
    let field = gid::new_cell_id();
    let session = session(Value::record([(field, text::value("hello"))]));
    let shown = session.borrow().driver.rect(&[Step::Key(field)]);
    assert_eq!(
        send(
            &session,
            call(LEFT_CLICK, [(COORDINATE, at(shown.center()))])
        ),
        Value::from(DONE)
    );
    assert_eq!(session.borrow().driver.selection(), [Step::Key(field)]);
}

#[test]
fn a_triple_click_selects_all_of_a_text() {
    let field = gid::new_cell_id();
    let session = session(Value::record([(field, text::value("hello world"))]));
    let shown = session.borrow().driver.text_at(&[Step::Key(field)]);
    assert_eq!(
        send(&session, call(TRIPLE_CLICK, [(COORDINATE, at(shown))])),
        Value::from(DONE)
    );
    assert_eq!(
        send(&session, call(TYPE, [(TEXT, text::value("bye"))])),
        Value::from(DONE)
    );
    assert_eq!(
        session.borrow().driver.value(&[Step::Key(field)]),
        text::value("bye")
    );
}

#[test]
fn actions_answer_why_they_could_not_happen() {
    let session = session(text::value("hello"));
    assert_eq!(
        send(
            &session,
            call(LEFT_CLICK, [(COORDINATE, at(Point::new(5000.0, 10.0)))])
        ),
        ::grap::absent::value(OUTSIDE_WINDOW)
    );
    assert_eq!(
        send(
            &session,
            call(LEFT_CLICK, [(COORDINATE, text::value("the middle"))])
        ),
        ::grap::absent::value(NOT_A_COORDINATE)
    );
    assert_eq!(
        send(&session, call(KEY, [(TEXT, text::value("hyper+q"))])),
        ::grap::absent::value(UNKNOWN_KEY)
    );
    assert!(::grap::absent::is_absent(&send(
        &session,
        call(MOUSE_MOVE, [])
    )));
}

#[test]
fn zooming_in_draws_larger_where_clicks_still_land() {
    let field = gid::new_cell_id();
    let session = session(Value::record([(field, text::value("hello"))]));
    let shown = |session: &Rc<RefCell<Session>>| session.borrow().driver.rect(&[Step::Key(field)]);
    let before = shown(&session);
    assert_eq!(send(&session, command("plus")), Value::from(DONE));
    let after = shown(&session);
    assert!(after.width() > before.width() && after.height() > before.height());
    assert_eq!(
        send(
            &session,
            call(LEFT_CLICK, [(COORDINATE, at(after.center()))])
        ),
        Value::from(DONE)
    );
    assert_eq!(session.borrow().driver.selection(), [Step::Key(field)]);
}

#[test]
fn scrolling_down_moves_a_long_document_up() {
    let session = session(Value::list(
        (0..200).map(|line| text::value(format!("line {line}"))),
    ));
    let first = |session: &Rc<RefCell<Session>>| {
        let driver = &session.borrow().driver;
        driver.rect(&driver.element(&[], 0, &[])).y0
    };
    let before = first(&session);
    assert_eq!(
        send(
            &session,
            call(
                SCROLL,
                [
                    (COORDINATE, at(Point::new(400.0, 400.0))),
                    (SCROLL_DIRECTION, text::value("down")),
                    (SCROLL_AMOUNT, f64::value(3.0)),
                ]
            )
        ),
        Value::from(DONE)
    );
    assert!(first(&session) < before);
}

#[test]
fn requests_that_are_not_documents_or_have_no_root_say_so() {
    let session = session(text::value("hello"));
    let binders = computer::binders();
    for request in [&b"{root: [}"[..], b"{}"] {
        let reply = respond(&Session::libraries(&session), &binders, request);
        let root = gid_text::parse(&reply).unwrap().0.root.unwrap();
        assert_eq!(::grap::absent::reason(&root), Some(INVALID_REQUEST));
    }
}

#[test]
fn a_document_ends_at_the_brace_that_closes_it() {
    assert_eq!(document_end(br#"{"root": {a: "}"}} {"#), Some(18));
    assert_eq!(document_end(br#"{"root": "\"}"}"#), Some(15));
    assert_eq!(document_end(b"{\"root\": {a: b}"), None);
    assert_eq!(document_end(b"  "), None);
}

#[test]
#[ignore = "needs a GPU"]
fn gpu_screenshots_are_pngs_of_the_window() {
    let session = session(text::value("hello"));
    let reply = send(&session, call(SCREENSHOT, []));
    let bytes = crate::libraries::png::read(&reply).expect("a png");
    assert_eq!(
        crate::libraries::png::size(bytes),
        Some((WINDOW.width as u32, WINDOW.height as u32))
    );
}
