use super::*;

#[test]
fn entries_sort_by_key_and_a_repeated_key_keeps_its_last_value() {
    assert_eq!(
        sorted_entries([(2, "first 2"), (1, "1"), (2, "last 2")]),
        [(1, "1"), (2, "last 2")]
    );
    assert_eq!(sorted_entries([(1, "1"), (2, "2")]), [(1, "1"), (2, "2")]);
}
