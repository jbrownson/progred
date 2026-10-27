//! Progred's value lens: pure rebuilds along a spine of Key and Element
//! steps within one value. Every editor write reduces to
//! splitting its path at the last Follow — the owning cell — and
//! rebuilding that cell's value along the remaining spine with these.
//! A Follow inside a spine crosses an identity boundary, which is the
//! caller's split point, never the lens's: it declines.

use gid::{Step, Value};

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
            let rebuilt = set(child, rest, leaf)?;
            Some(Value::Record(fields.with(*label, rebuilt)))
        }
        Some((Step::Element(position), rest)) => {
            let elements = current?.as_list()?;
            let child = elements.get(position);
            let rebuilt = set(child, rest, leaf)?;
            Some(Value::List(elements.with(position.clone(), rebuilt)))
        }
        Some((Step::Follow(_), _)) => None,
    }
}

/// Rebuilds `value` without the field or element at the spine's final
/// step. Declines when the spine is empty (removal at nothing) or the
/// target is absent, keeping no-op writes distinguishable.
pub fn without(value: &Value, spine: &[Step]) -> Option<Value> {
    match spine.split_first() {
        None => None,
        Some((Step::Key(label), [])) => {
            let mut fields = value.as_record()?.clone();
            fields.set(*label, None)?;
            Some(Value::Record(fields))
        }
        Some((Step::Element(position), [])) => {
            let mut elements = value.as_list()?.clone();
            elements.set(position.clone(), None)?;
            Some(Value::List(elements))
        }
        Some((Step::Key(label), rest)) => {
            let fields = value.as_record()?;
            let rebuilt = without(fields.get(label)?, rest)?;
            Some(Value::Record(fields.with(*label, rebuilt)))
        }
        Some((Step::Element(position), rest)) => {
            let elements = value.as_list()?;
            let rebuilt = without(elements.get(position)?, rest)?;
            Some(Value::List(elements.with(position.clone(), rebuilt)))
        }
        Some((Step::Follow(_), _)) => None,
    }
}

#[cfg(test)]
#[path = "tests/spine.rs"]
mod tests;
