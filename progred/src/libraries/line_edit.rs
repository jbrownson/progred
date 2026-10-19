//! The conversion an atomic-value library's line control writes back
//! through. A library describes its line as a `LineEdit`: spelling,
//! affixes, family, and this native callback from typed spelling to value.

use crate::display::LineUpdate;
use gid::Value;
use std::rc::Rc;

pub fn native(update: impl Fn(&str, Option<&Value>) -> Option<Value> + 'static) -> LineUpdate {
    Rc::new(update)
}
