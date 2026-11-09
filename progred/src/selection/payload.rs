use ::grap::f64 as f64_convention;
use ::grap::text;
use gid::{CellId, Value};
use kurbo::Point;
use puri::edit::LineEditState;

pub mod vocabulary {
    use gid::CellId;

    /// A selection's stage: an edge, or a pending value or label being
    /// typed. The editor library names these.
    pub const STAGE: CellId = CellId::from_u128(0x6a1fd3082b9c47e5f60d21a8c45e9b37);
    pub const EDGE: CellId = CellId::from_u128(0x2f74c8a1936e05bd4c17e2b98d60a5f4);
    pub const PENDING: CellId = CellId::from_u128(0x91d5e60b3a8f27c4058b39f6d2c471ea);
    pub const LABEL: CellId = CellId::from_u128(0x7be29f4680d1c5a3f2496e07b85d13c2);
    pub const QUERY: CellId = CellId::from_u128(0xc25e80f7d1934ab6270c8f5e13b6d4a9);
    pub const CHOICE: CellId = CellId::from_u128(0x48b7a92c05e1d6f3891a4d20e7c53f6b);
    pub const COMPLETION_SCROLL: CellId = CellId::from_u128(0x151767a413a8bc5f579465dd67f18263);

    /// Selection byte offsets; FOCUS may precede ANCHOR.
    pub const ANCHOR: CellId = CellId::from_u128(0x5d38a1c7f24e9b60d15c7a02e83f46b9);
    pub const FOCUS: CellId = CellId::from_u128(0xa906e35d21c84f7bc3d05e918b62fa47);
    /// The in-flight IME composition: a text value with optional
    /// START/END cursor fields overlaid.
    pub const PREEDIT: CellId = CellId::from_u128(0x1c84f0b6d97325ea40d6b18c53e29f74);
    pub const START: CellId = CellId::from_u128(0xf27b950e13a8d64c26f9e30a71d45b8c);
    pub const END: CellId = CellId::from_u128(0x60d3e94a852f17bd39c2a45f08e61d73);
    /// The editor's in-motion text. For a pending this mirrors
    /// QUERY; for an edge it preserves text until write-through.
    pub const EDITOR_TEXT: CellId = CellId::from_u128(0x3b3544bd8a2fc3a83a08edb6766fad4a);
    /// The in-progress drag-selection: window origin and click count.
    pub const DRAG: CellId = CellId::from_u128(0xb49c26e1075df3a8e5017d29c46b83f5);
    pub const X: CellId = CellId::from_u128(0x39e50d7ac1846f2b7a2384b06d95c1ef);
    pub const Y: CellId = CellId::from_u128(0x8e17b3f4692a05dc90e5f6c2374a18db);
    pub const COUNT: CellId = CellId::from_u128(0x4dab72e9508c31f6cb490271f8ea56d0);
}

pub fn edge() -> Value {
    Value::record([(vocabulary::STAGE, Value::Cell(vocabulary::EDGE))])
}

/// A pending with nothing typed yet.
pub fn empty_pending() -> Value {
    Value::record([(vocabulary::STAGE, Value::Cell(vocabulary::PENDING))])
}

/// A pending value with `query` typed, its first offer chosen.
pub fn pending(query: &str) -> Value {
    Value::record([
        (vocabulary::STAGE, Value::Cell(vocabulary::PENDING)),
        (vocabulary::QUERY, text::value(query)),
        (vocabulary::CHOICE, f64_convention::value(0.0)),
    ])
}

/// A new field's label with `query` typed, its first offer chosen.
pub fn label(query: &str) -> Value {
    Value::record([
        (vocabulary::STAGE, Value::Cell(vocabulary::LABEL)),
        (vocabulary::QUERY, text::value(query)),
        (vocabulary::CHOICE, f64_convention::value(0.0)),
    ])
}

pub fn stage(payload: &Value) -> Option<CellId> {
    payload.as_record()?.get(&vocabulary::STAGE)?.as_cell()
}

pub fn query(payload: &Value) -> Option<&str> {
    text::read(payload.as_record()?.get(&vocabulary::QUERY)?)
}

pub fn choice(payload: &Value) -> Option<usize> {
    index(payload.as_record()?.get(&vocabulary::CHOICE)?)
}

/// The whole, non-negative number `value` spells, as an index or offset.
fn index(value: &Value) -> Option<usize> {
    f64_convention::read(value).and_then(f64_convention::to_usize)
}

pub fn completion_scroll(payload: &Value) -> Option<f64> {
    f64_convention::read(payload.as_record()?.get(&vocabulary::COMPLETION_SCROLL)?)
        .filter(|scroll| scroll.is_finite() && *scroll >= 0.0)
}

pub fn with_completion_query(payload: &Value, query: &str) -> Value {
    if self::query(payload) == Some(query) {
        payload.clone()
    } else {
        with_field(
            &with_completion_view(payload, 0.0, 0),
            vocabulary::QUERY,
            text::value(query),
        )
    }
}

pub fn with_completion_view(payload: &Value, scroll: f64, choice: usize) -> Value {
    let fields = payload.as_record().cloned().unwrap_or_default();
    Value::Record(
        fields
            .with(vocabulary::COMPLETION_SCROLL, f64_convention::value(scroll))
            .with(vocabulary::CHOICE, f64_convention::value(choice as f64)),
    )
}

pub fn without_editor(payload: &Value) -> Value {
    let mut fields = payload.as_record().cloned().unwrap_or_default();
    for field in [
        vocabulary::EDITOR_TEXT,
        vocabulary::ANCHOR,
        vocabulary::FOCUS,
        vocabulary::PREEDIT,
        vocabulary::DRAG,
    ] {
        fields.set(field, None);
    }
    Value::Record(fields)
}

fn with_field(payload: &Value, key: CellId, value: Value) -> Value {
    let fields = payload.as_record().cloned().unwrap_or_default();
    Value::Record(fields.with(key, value))
}

/// Encode the whole live editor into the payload: its in-motion
/// text, the query text when the stage owns it, selection offsets,
/// and any in-flight IME composition or drag.
pub fn with_editor(payload: &Value, line: &LineEditState, own_text: bool) -> Value {
    let payload = if own_text {
        with_completion_query(payload, line.text())
    } else {
        payload.clone()
    };
    let mut fields = payload.as_record().cloned().unwrap_or_default();
    fields.set(vocabulary::EDITOR_TEXT, text::value(line.text()));
    let (anchor, focus) = line.selection_offsets();
    fields.set(vocabulary::ANCHOR, f64_convention::value(anchor as f64));
    fields.set(vocabulary::FOCUS, f64_convention::value(focus as f64));
    let preedit = line.preedit_parts().map(|(preedit, cursor)| {
        let mut composed = text::value(preedit)
            .as_record()
            .cloned()
            .unwrap_or_default();
        if let Some((start, end)) = cursor {
            composed.set(vocabulary::START, f64_convention::value(start as f64));
            composed.set(vocabulary::END, f64_convention::value(end as f64));
        }
        Value::Record(composed)
    });
    fields.set(vocabulary::PREEDIT, preedit);
    let drag = line.drag_parts().map(|(origin, count)| {
        Value::record([
            (vocabulary::X, f64_convention::value(origin.x)),
            (vocabulary::Y, f64_convention::value(origin.y)),
            (vocabulary::COUNT, f64_convention::value(f64::from(count))),
        ])
    });
    fields.set(vocabulary::DRAG, drag);
    Value::Record(fields)
}

pub fn editor_text(payload: &Value) -> Option<&str> {
    text::read(payload.as_record()?.get(&vocabulary::EDITOR_TEXT)?)
}

/// Decode the live editor from the payload over fallback `text`.
/// An encoded in-motion spelling wins when present.
/// Absent offsets land the caret at the end (the mount default);
/// junk clamps, per [`LineEditState::from_parts`].
pub fn editor_line(payload: &Value, text: &str) -> LineEditState {
    let text = editor_text(payload).unwrap_or(text);
    let field_index = |key: CellId| index(payload.as_record()?.get(&key)?);
    let anchor = field_index(vocabulary::ANCHOR).unwrap_or(text.len());
    let focus = field_index(vocabulary::FOCUS).unwrap_or(text.len());
    let preedit = payload
        .as_record()
        .and_then(|fields| fields.get(&vocabulary::PREEDIT))
        .and_then(|composed| {
            let cursor = composed.as_record().and_then(|fields| {
                let offset = |key: CellId| index(fields.get(&key)?);
                Some((offset(vocabulary::START)?, offset(vocabulary::END)?))
            });
            Some((text::read(composed)?.to_string(), cursor))
        });
    let drag = payload
        .as_record()
        .and_then(|fields| fields.get(&vocabulary::DRAG))
        .and_then(Value::as_record)
        .and_then(|drag| {
            let coordinate = |key: CellId| f64_convention::read(drag.get(&key)?);
            Some((
                Point::new(coordinate(vocabulary::X)?, coordinate(vocabulary::Y)?),
                coordinate(vocabulary::COUNT)? as u8,
            ))
        });
    LineEditState::from_parts(text, anchor, focus, preedit, drag)
}

#[cfg(test)]
#[path = "tests/payload.rs"]
mod tests;
