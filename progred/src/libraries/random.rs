//! Reproducible random numbers, as pure functions of a key. A key is a `u64`.
//! `random key` derives another from it and an index, so each part of a
//! computation, such as each branch of a tree, has its own, and `random
//! between` draws a number from a key and an index. The same key and index
//! always give the same number, and drawing one changes no other.

use crate::display::Host;
use crate::libraries::{Library, name};
use ::grap::{ForeignFunction, ForeignFunctions, RuntimeValue};

pub const ID: gid::CellId = gid::CellId::from_u128(0x953e2838d5985718dd4e91f0af673fe3);

pub mod vocabulary {
    use gid::CellId;

    pub const BETWEEN: CellId = CellId::from_u128(0xe8a99ada91800bca33709da0db80c29f);
    pub const DERIVE: CellId = CellId::from_u128(0xf623f74deb375d0c1476ed4baed4bd3c);
    pub const KEY: CellId = CellId::from_u128(0x78727d568538f936fcb3e28c5961d0ca);
    pub const INDEX: CellId = CellId::from_u128(0xb71da70705ea6085786117b5c47525e7);
    pub const MIN: CellId = CellId::from_u128(0x61213005b01abca2d4fb351cc4fe4d98);
    pub const MAX: CellId = CellId::from_u128(0x4be40301b427aeed939ef079ab63cf92);
}

/// SplitMix64's output function: a well-mixed `u64` from any other.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// `key` and `index` folded together in `domain`, so a key and a draw made
/// from the same key and index differ.
fn fold(key: u64, index: f64, domain: u64) -> u64 {
    mix(key ^ mix(index.to_bits().wrapping_add(domain)))
}

/// The key for part `index` of what `key` draws.
pub(crate) fn derive(key: u64, index: f64) -> u64 {
    fold(key, index, 0x9e37_79b9_7f4a_7c15)
}

/// A number from `min` up to `max`, drawn from `key` at `index`.
pub(crate) fn between(key: u64, index: f64, min: f64, max: f64) -> f64 {
    let unit = (fold(key, index, 0x3c6e_f372_fe94_f82a) >> 11) as f64 / (1_u64 << 53) as f64;
    min + (max - min) * unit
}

fn functions() -> ForeignFunctions<'static> {
    use vocabulary::*;
    ForeignFunctions::default()
        .register(
            DERIVE,
            ForeignFunction::with_arguments([KEY, INDEX], |_, (key, index): (u64, f64)| {
                RuntimeValue::u64(derive(key, index))
            })
            .cacheable(),
        )
        .register(
            BETWEEN,
            ForeignFunction::with_arguments(
                [KEY, INDEX, MIN, MAX],
                |_, (key, index, min, max): (u64, f64, f64, f64)| {
                    RuntimeValue::f64(between(key, index, min, max))
                },
            )
            .cacheable(),
        )
}

pub fn library<W: Host>() -> Library<W> {
    use vocabulary::*;
    name::named_library(
        ID,
        "random",
        [
            (BETWEEN, "random between"),
            (DERIVE, "random key"),
            (KEY, "key"),
            (INDEX, "index"),
            (MIN, "minimum"),
            (MAX, "maximum"),
        ],
        functions(),
    )
}

#[cfg(test)]
#[path = "tests/random.rs"]
mod tests;
