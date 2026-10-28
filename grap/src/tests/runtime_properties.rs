//! A value held as stored GID data and the same value built at runtime agree
//! on every read, convert back to the same GID, and evaluate alike.

use crate::eval::unbox;
use crate::runtime::RuntimeValueKind;
use crate::{RuntimeValue, Shape, SourceOrigin, TestHost, isa, vocabulary};
use gid::{CellId, Position, Value};
use proptest::prelude::*;

const A: CellId = CellId::from_u128(0x5e1f0c6a1b2d4e8f9a0b1c2d3e4f5a6b);
const B: CellId = CellId::from_u128(0x7c3a9e1d5b2f4a6c8e0d1f3b5a7c9e2d);

/// Labels a record may have: plain ones, the conventions' and Grap's own.
const LABELS: [CellId; 12] = [
    A,
    B,
    isa::ISA,
    crate::f64::F64,
    crate::f32::F32,
    crate::u64::U64,
    crate::text::UTF8,
    crate::logic::BOOL,
    vocabulary::FUNCTION,
    vocabulary::PARAMS,
    vocabulary::BODY,
    vocabulary::VALUE,
];

/// Cells a reference may name: the labels, and the kinds and booleans the
/// conventions recognize.
fn cell() -> impl Strategy<Value = CellId> {
    prop_oneof![
        prop::sample::select(LABELS.to_vec()),
        prop::sample::select(vec![
            vocabulary::kind::CALL,
            vocabulary::kind::LAMBDA,
            vocabulary::kind::VALUE_WRAPPER,
            vocabulary::kind::CLOSURE,
            vocabulary::kind::ABSENT,
            crate::logic::TRUE,
            crate::logic::FALSE,
        ]),
    ]
}

fn leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        cell().prop_map(Value::from),
        // Lengths 4 and 8 can decode as numbers; short ASCII as text.
        prop::collection::vec(any::<u8>(), 0..10).prop_map(Value::from),
        any::<f64>().prop_map(crate::f64::value),
        any::<f32>().prop_map(crate::f32::value),
        any::<u64>().prop_map(crate::u64::value),
        "[a-z]{0,4}".prop_map(crate::text::value),
        any::<bool>().prop_map(crate::logic::value),
    ]
}

/// A list built by inserting each element at a chosen place, so its
/// positions are ones editing makes rather than freshly minted ones.
fn edited_list(insertions: Vec<(u8, Value)>) -> Value {
    let mut entries: Vec<(Position, Value)> = Vec::new();
    for (place, value) in insertions {
        let index = usize::from(place) % (entries.len() + 1);
        let low = index.checked_sub(1).map(|low| &entries[low].0);
        let high = entries.get(index).map(|(position, _)| position);
        let position = gid::position::between(low, high).expect("neighbours are ordered");
        entries.insert(index, (position, value));
    }
    Value::List(entries.into_iter().collect())
}

fn value() -> impl Strategy<Value = Value> {
    leaf().prop_recursive(3, 32, 4, |inner| {
        prop_oneof![
            prop::collection::vec((prop::sample::select(LABELS.to_vec()), inner.clone()), 0..5)
                .prop_map(Value::record),
            prop::collection::vec((any::<u8>(), inner), 0..5).prop_map(edited_list),
        ]
    })
}

/// `value` as evaluation builds values: numbers, text and booleans
/// unboxed, other records and lists as runtime containers.
fn built(value: &Value) -> RuntimeValue {
    match value {
        Value::Record(fields) => match unbox(value.into()) {
            unboxed @ RuntimeValue(RuntimeValueKind::Unboxed(_), _) => unboxed,
            _ => RuntimeValue::record(fields.iter().map(|(label, value)| (*label, built(value)))),
        },
        Value::List(elements) => RuntimeValue::list_at(
            elements.keys().cloned().collect(),
            elements.values().map(built),
        ),
        Value::Cell(_) | Value::Blob(_) => value.into(),
    }
}

/// Equal, with every list in them at the same positions: GID's own
/// equality leaves positions out.
fn identical(a: &Value, b: &Value) -> bool {
    a == b
        && match (a, b) {
            (Value::List(a), Value::List(b)) => a
                .iter()
                .zip(b.iter())
                .all(|((p, x), (q, y))| p == q && identical(x, y)),
            (Value::Record(a), Value::Record(b)) => {
                a.values().zip(b.values()).all(|(x, y)| identical(x, y))
            }
            _ => true,
        }
}

fn shape(value: &RuntimeValue) -> String {
    match value.shape() {
        Shape::Blob(bytes) => format!("blob {bytes:?}"),
        Shape::Cell(cell) => format!("cell {cell}"),
        Shape::List(positions) => format!("list {positions:?}"),
        Shape::Record(labels) => format!("record {labels:?}"),
    }
}

/// Every read of `stored` and `built` gives the same answer, down through
/// their fields and elements.
fn agree(stored: &RuntimeValue, built: &RuntimeValue) -> Result<(), TestCaseError> {
    prop_assert!(identical(stored.as_value(), built.as_value()));
    prop_assert_eq!(shape(stored), shape(built));
    prop_assert_eq!(stored.as_cell(), built.as_cell());
    prop_assert_eq!(stored.as_blob(), built.as_blob());
    prop_assert_eq!(
        stored.as_f64().map(f64::to_bits),
        built.as_f64().map(f64::to_bits)
    );
    prop_assert_eq!(
        stored.as_f32().map(f32::to_bits),
        built.as_f32().map(f32::to_bits)
    );
    prop_assert_eq!(stored.as_u64(), built.as_u64());
    prop_assert_eq!(stored.as_text(), built.as_text());
    prop_assert_eq!(stored.kind(), built.kind());
    prop_assert_eq!(stored.record_len(), built.record_len());
    prop_assert_eq!(stored.record_keys(), built.record_keys());
    for label in LABELS {
        prop_assert_eq!(stored.contains_field(label), built.contains_field(label));
        match (stored.field(label), built.field(label)) {
            (Some(stored), Some(built)) => agree(&stored, &built)?,
            (None, None) => {}
            (stored, built) => prop_assert!(
                false,
                "field {label}: {:?} against {:?}",
                stored.map(RuntimeValue::into_value),
                built.map(RuntimeValue::into_value),
            ),
        }
    }
    prop_assert_eq!(stored.list_len(), built.list_len());
    let positions = stored.list_positions();
    prop_assert_eq!(&positions, &built.list_positions());
    for (index, position) in positions.iter().flatten().enumerate() {
        agree(
            &stored.list_get(index).unwrap(),
            &built.list_get(index).unwrap(),
        )?;
        match built.list_element(position) {
            Some(element) => agree(&stored.list_element(position).unwrap(), &element)?,
            None => prop_assert!(false, "no element at {position:?}"),
        }
    }
    Ok(())
}

proptest! {
    #[test]
    fn a_built_value_converts_back_to_the_same_gid(value in value()) {
        prop_assert!(identical(&built(&value).to_value(), &value));
        prop_assert!(identical(&built(&value).into_value(), &value));
    }

    #[test]
    fn a_built_value_reads_like_the_stored_one(value in value()) {
        agree(&RuntimeValue::from(&value), &built(&value))?;
    }

    #[test]
    fn a_new_list_reads_at_the_positions_it_converts_to(
        values in prop::collection::vec(value(), 0..6),
    ) {
        let list = RuntimeValue::list(values.iter().map(built));
        agree(&RuntimeValue::from(list.to_value()), &list)?;
    }

    #[test]
    fn code_evaluates_alike_stored_or_built(value in value()) {
        let host = TestHost(|_| None);
        let stored = crate::evaluate_value(&value, &host, 1_000);
        let built = crate::evaluate_runtime_at(
            &built(&value),
            Some(SourceOrigin::Input(Vec::new())),
            &host,
            1_000,
        )
        .into_value();
        prop_assert!(identical(&stored.result, &built.result), "{stored:?} against {built:?}");
        prop_assert_eq!(stored, built);
    }
}
