use super::*;
use crate::projection::Layout;
use crate::projection::test_support::ProjectionInput;

use crate::display::test_support::NoEval;
use crate::libraries::test_widgets::projection_input;
use gid::{Value, new_cell_id};

fn functions() -> ::grap::ForeignFunctions {
    crate::test_support::foreign_functions(&library::<crate::Editor>())
}

#[test]
fn numeric_projections_read_runtime_facets_and_ignore_unrelated_children() {
    type Project = for<'a, 'b> fn(&'b ProjectionInput<'a, RuntimeValue>) -> Option<Layout>;
    let extra = new_cell_id();
    let functions = functions();
    let host = crate::test_support::test_host(|_| None, &functions);
    let callback = ::grap::evaluate(&::grap::lambda([], value(1.0)), &host, 100).result;
    for (field, bytes, project) in [
        (
            vocabulary::F64,
            3.5_f64.to_le_bytes().to_vec(),
            (|input| representation().display(input)) as Project,
        ),
        (
            crate::libraries::f32::vocabulary::F32,
            3.5_f32.to_le_bytes().to_vec(),
            |input| crate::libraries::f32::representation().display(input),
        ),
        (
            crate::libraries::u64::vocabulary::U64,
            42_u64.to_le_bytes().to_vec(),
            |input| crate::libraries::u64::representation().display(input),
        ),
    ] {
        let number = RuntimeValue::record([
            (field, Value::from(bytes).into()),
            (extra, callback.clone()),
        ]);
        assert!(project(&projection_input(&NoEval, &number)).is_some());
        assert!(number.field(extra).unwrap().same_result(&callback));
        let invalid = RuntimeValue::record([(field, Value::from(vec![0]).into())]);
        assert!(project(&projection_input(&NoEval, &invalid)).is_none());
    }
    assert!(
        representation()
            .display(&projection_input(&NoEval, &RuntimeValue::f64(3.5)))
            .is_some()
    );
}

#[test]
fn representation_is_library_data() {
    assert_eq!(read(&value(2.5)), Some(2.5));
    assert_eq!(read(&Value::from(b"2.5".to_vec())), None);

    let extra = new_cell_id();
    let with_extra = Value::record(
        value(2.5)
            .as_record()
            .unwrap()
            .clone()
            .update(extra, Value::from(b"degrees".to_vec())),
    );
    assert_eq!(read(&with_extra), Some(2.5));
    let update = |input: &str| number::edit(input, Some(&with_extra), value);
    // Unparseable input declines the write whole.
    assert_eq!(update("junk"), None);
    assert_eq!(
        update("3"),
        Some(Value::record(
            value(3.0)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"degrees".to_vec())),
        ))
    );
}

#[test]
fn pi_reads_as_its_number_and_its_name() {
    let library = library::<crate::Editor>();
    let pi = library.definitions.value(vocabulary::PI).unwrap();
    assert_eq!(read(pi), Some(std::f64::consts::PI));
    assert_eq!(name::read(pi), Some("π"));
}
