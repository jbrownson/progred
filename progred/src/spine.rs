//! Progred's value lens: pure reads and rebuilds along a spine of Key and
//! Element steps within one value. Every editor write reduces to
//! splitting its path at the last Follow — the owning cell — and
//! rebuilding that cell's value along the remaining spine with these.
//! A Follow inside a spine crosses an identity boundary, which is the
//! caller's split point, never the lens's: it declines.

use gid::{Step, Value};

#[cfg(test)]
pub fn get<'a>(value: &'a Value, spine: &[Step]) -> Option<&'a Value> {
    spine.iter().try_fold(value, |value, step| match step {
        Step::Key(label) => value.as_record()?.get(label),
        Step::Element(position) => value.as_list()?.get(position),
        Step::Follow => None,
    })
}

/// Rebuilds `current` along the spine, replacing the leaf: surviving
/// structure is shared, and the final step inserts or replaces at its
/// label or position. Deeper steps need existing structure to descend
/// through; a missing container declines.
pub fn set(current: Option<&Value>, spine: &[Step], leaf: Value) -> Option<Value> {
    match spine.split_first() {
        None => Some(leaf),
        Some((Step::Key(label), rest)) => {
            let fields = current?.as_record()?;
            let child = fields.get(label);
            if !rest.is_empty() && child.is_none() {
                return None;
            }
            let rebuilt = set(child, rest, leaf)?;
            Some(Value::Record(fields.update(*label, rebuilt)))
        }
        Some((Step::Element(position), rest)) => {
            let elements = current?.as_list()?;
            let child = elements.get(position);
            if !rest.is_empty() && child.is_none() {
                return None;
            }
            let rebuilt = set(child, rest, leaf)?;
            Some(Value::List(elements.update(position.clone(), rebuilt)))
        }
        Some((Step::Follow, _)) => None,
    }
}

/// Rebuilds `value` without the field or element at the spine's final
/// step. Declines when the spine is empty (removal at nothing) or the
/// target is absent, keeping no-op writes distinguishable.
pub fn without(value: &Value, spine: &[Step]) -> Option<Value> {
    match spine.split_first() {
        None => None,
        Some((Step::Key(label), [])) => {
            let fields = value.as_record()?;
            fields
                .contains_key(label)
                .then(|| Value::Record(fields.without(label)))
        }
        Some((Step::Element(position), [])) => {
            let elements = value.as_list()?;
            elements
                .contains_key(position)
                .then(|| Value::List(elements.without(position)))
        }
        Some((Step::Key(label), rest)) => {
            let fields = value.as_record()?;
            let rebuilt = without(fields.get(label)?, rest)?;
            Some(Value::Record(fields.update(*label, rebuilt)))
        }
        Some((Step::Element(position), rest)) => {
            let elements = value.as_list()?;
            let rebuilt = without(elements.get(position)?, rest)?;
            Some(Value::List(elements.update(position.clone(), rebuilt)))
        }
        Some((Step::Follow, _)) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{CellId, position};

    fn relation(name: &str) -> CellId {
        CellId::from_u128(match name {
            "name" => 0x7a9138f84ed69a4546f2cd53021a7364,
            "points" => 0x2e20418e4f70b7fbd650cd6675ab920a,
            "row" => 0xe56ef28a80053b2493bd422dbb34e9f4,
            "missing" => 0x87e099e7f26bc29933470066a4d9bd3c,
            "deeper" => 0xa17a00a2df132d27f03dab7bd997fabb,
            "color" => 0x04488199db044b9c5da89f2b6db1a83c,
            "x" => 0x6b040071381df24022a370d51793e9cd,
            "only" => 0xe923cf979e6706d090d6b56ef9e2fc88,
            _ => unreachable!("fixture relation"),
        })
    }

    fn label(name: &str) -> CellId {
        relation(name)
    }

    fn blob(text: &str) -> Value {
        Value::from(text.as_bytes().to_vec())
    }

    fn key(s: &str) -> Step {
        Step::Key(label(s))
    }

    fn sample() -> Value {
        Value::record([
            (label("name"), blob("roof")),
            (
                label("points"),
                Value::list([Value::record([(label("row"), blob("top"))]), blob("loose")]),
            ),
        ])
    }

    fn element_at(value: &Value, spine: &[Step], index: usize) -> Step {
        Step::Element(
            get(value, spine)
                .unwrap()
                .as_list()
                .unwrap()
                .keys()
                .nth(index)
                .unwrap()
                .clone(),
        )
    }

    #[test]
    fn get_walks_keys_and_elements() {
        let value = sample();
        let first = element_at(&value, &[key("points")], 0);
        assert_eq!(
            get(&value, &[key("points"), first.clone(), key("row")]),
            Some(&blob("top"))
        );
        assert_eq!(get(&value, &[key("missing")]), None);
        assert_eq!(get(&value, &[key("name"), key("deeper")]), None);
        assert_eq!(get(&value, &[Step::Follow]), None);
    }

    #[test]
    fn set_replaces_inserts_and_rebuilds_deeply() {
        let value = sample();
        let first = element_at(&value, &[key("points")], 0);

        // Replace a leaf deep inside; siblings and positions survive.
        let deep = [key("points"), first.clone(), key("row")];
        let rebuilt = set(Some(&value), &deep, blob("bottom")).unwrap();
        assert_eq!(get(&rebuilt, &deep), Some(&blob("bottom")));
        assert_eq!(get(&rebuilt, &[key("name")]), Some(&blob("roof")));
        assert_eq!(
            get(&value, &[key("points")])
                .unwrap()
                .as_list()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            get(&rebuilt, &[key("points")])
                .unwrap()
                .as_list()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
        );

        // The final step inserts: a fresh field, a fresh position.
        let added = set(Some(&value), &[key("color")], blob("red")).unwrap();
        assert_eq!(get(&added, &[key("color")]), Some(&blob("red")));
        let last = match element_at(&value, &[key("points")], 1) {
            Step::Element(p) => p,
            _ => unreachable!(),
        };
        let fresh = position::between(Some(&last), None).unwrap();
        let appended = set(
            Some(&value),
            &[key("points"), Step::Element(fresh.clone())],
            blob("tail"),
        )
        .unwrap();
        assert_eq!(
            get(&appended, &[key("points"), Step::Element(fresh)]),
            Some(&blob("tail"))
        );

        // Deeper steps need existing structure; Follow is the
        // caller's boundary.
        assert!(set(Some(&value), &[key("missing"), key("x")], blob("v")).is_none());
        assert!(set(Some(&value), &[key("name"), key("x")], blob("v")).is_none());
        assert!(set(Some(&value), &[Step::Follow], blob("v")).is_none());
        assert!(set(None, &[key("x")], blob("v")).is_none());
        // An empty spine authors the value whole — a bare cell's
        // first value, the root's replacement.
        assert_eq!(set(None, &[], blob("v")), Some(blob("v")));
    }

    #[test]
    fn without_removes_fields_and_elements() {
        let value = sample();
        let first = element_at(&value, &[key("points")], 0);

        let no_name = without(&value, &[key("name")]).unwrap();
        assert_eq!(get(&no_name, &[key("name")]), None);
        assert!(get(&no_name, &[key("points")]).is_some());

        let no_first = without(&value, &[key("points"), first.clone()]).unwrap();
        assert_eq!(
            get(&no_first, &[key("points")])
                .unwrap()
                .as_list()
                .unwrap()
                .len(),
            1
        );

        // Removing inside a nested record rebuilds the spine above.
        let no_row = without(&value, &[key("points"), first.clone(), key("row")]).unwrap();
        assert_eq!(get(&no_row, &[key("points"), first, key("row")]), None);
        assert!(get(&no_row, &[key("name")]).is_some());

        assert!(without(&value, &[]).is_none());
        assert!(without(&value, &[key("missing")]).is_none());
        assert!(without(&value, &[Step::Follow]).is_none());
    }

    #[test]
    fn emptied_containers_persist_as_values() {
        // No sticky-kind machinery: an emptied record is still the
        // empty record, an emptied list the empty list.
        let record = Value::record([(label("only"), blob("x"))]);
        assert_eq!(without(&record, &[key("only")]), Some(Value::record([])));
        let list = Value::list([blob("x")]);
        let sole = element_at(&list, &[], 0);
        assert_eq!(without(&list, &[sole]), Some(Value::list([])));
    }
}
