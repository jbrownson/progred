//! What every number representation shares: the argument labels of
//! binary and unary operations, and the scrub editing helper. The
//! operation identities stay with each representation until dispatch
//! evaluates arguments once per call.

use crate::{Library, line_edit, name};
use gid::{CellId, Cells, Value};
use progred_display::{
    Face, Layout, ProjectionInput, ScrubEvent, ScrubUpdate, on_scrub, overlay_value, row, subscript,
};
use std::fmt::Display;
use std::rc::Rc;

pub const ID: CellId = CellId::from_u128(0xc46d010325d3a1ec0f2a84dd3a9570ae);

pub mod vocabulary {
    use gid::CellId;

    pub const LEFT: CellId = CellId::from_u128(0x764f6afe17ba14e81f5ab61204be0bec);
    pub const RIGHT: CellId = CellId::from_u128(0x4f53ff25390f58472d31a6142644dec2);
    pub const OPERAND: CellId = CellId::from_u128(0x50a20d15e4ae56be51b882de9d58c676);
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
        (vocabulary::OPERAND, "operand"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "number",
        crate::Definitions::from_parts(cells, Default::default()),
        progred_display::partial(|_| None),
    )
}

pub(crate) fn completions<N: std::str::FromStr + Display>(
    query: &str,
    representation: &str,
    encode: impl FnOnce(N) -> Value,
) -> Vec<progred_display::Completion> {
    match query.trim() {
        "" => "0",
        number => number,
    }
    .parse::<N>()
    .ok()
    .map(|number| {
        progred_display::Completion::new(number.to_string(), encode(number))
            .with_aliases([query])
            .with_detail(representation)
    })
    .into_iter()
    .collect()
}

const PIXELS_PER_STEP: f64 = 4.0;
const PIXELS_PER_DECADE: f64 = 24.0;
const DECADE_STRETCH: f64 = 1.5;

pub(crate) trait Scrubbable: Copy + Display + PartialOrd + 'static {
    fn magnitude(self) -> f64;
    fn minimum_precision() -> f64;
    fn scrubbable(self) -> bool;
    fn from_offset(start: Self, offset: f64, precision: f64) -> Self;
    fn spelling(self, precision: f64) -> String;
}

pub(crate) fn layout<World, Hover: Clone, N: Scrubbable>(
    input: &ProjectionInput<'_, World, Hover>,
    number: N,
    representation: &str,
    update: CellId,
    encode: fn(N) -> Value,
) -> Layout<World, Hover> {
    let line = row(
        2.0,
        [
            line_edit::layout(number.to_string(), grap_runtime::ffi(update), "", ""),
            subscript(representation, Face::Dim),
        ],
    );
    if !number.scrubbable() {
        return line;
    }
    let original = input.value.clone();
    let target = input.targets.current();
    on_scrub(
        line,
        target.hover,
        Rc::new(move || {
            let original = original.clone();
            let mut scrub = NumberScrub::new(number);
            Box::new(move |event| {
                let scrubbed = scrub.update(event);
                ScrubUpdate {
                    value: overlay_value(&original, encode(scrubbed.value)),
                    spelling: Some(scrubbed.value.spelling(scrubbed.precision)),
                }
            })
        }),
    )
}

struct Scrubbed<N> {
    value: N,
    precision: f64,
}

struct NumberScrub<N> {
    start: N,
    base: f64,
    offset: f64,
    displayed: N,
}

impl<N: Scrubbable> NumberScrub<N> {
    fn new(start: N) -> Self {
        Self {
            start,
            base: initial_precision(start.magnitude(), N::minimum_precision()),
            offset: 0.0,
            displayed: start,
        }
    }

    fn update(&mut self, event: ScrubEvent) -> Scrubbed<N> {
        let gain = 10.0_f64.powf(vertical_decades(event.distance_y).clamp(-16.0, 16.0));
        let scale = (self.base * gain).max(N::minimum_precision());
        let precision = nice_precision(scale);
        let horizontal_scale = scale / gain.max(1.0).cbrt();
        self.offset += event.movement_x * horizontal_scale / PIXELS_PER_STEP;
        let candidate = N::from_offset(self.start, self.offset, precision);
        self.displayed = if event.movement_x > 0.0 {
            partial_max(self.displayed, candidate)
        } else if event.movement_x < 0.0 {
            partial_min(self.displayed, candidate)
        } else {
            self.displayed
        };
        Scrubbed {
            value: self.displayed,
            precision,
        }
    }
}

fn partial_max<N: Copy + PartialOrd>(left: N, right: N) -> N {
    if right > left { right } else { left }
}

fn partial_min<N: Copy + PartialOrd>(left: N, right: N) -> N {
    if right < left { right } else { left }
}

fn initial_precision(magnitude: f64, minimum: f64) -> f64 {
    (if magnitude == 0.0 {
        0.01
    } else {
        10.0_f64
            .powf(magnitude.abs().log10().floor() - 2.0)
            .min(1.0)
    })
    .max(minimum)
}

fn vertical_decades(distance_y: f64) -> f64 {
    let distance = distance_y.abs();
    let decades = (1.0 + (DECADE_STRETCH - 1.0) * distance / PIXELS_PER_DECADE).log(DECADE_STRETCH);
    -distance_y.signum() * decades
}

fn nice_precision(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        let magnitude = 10.0_f64.powf(scale.log10().floor());
        let normalized = scale / magnitude;
        let coefficient = if normalized < 2.0 {
            1.0
        } else if normalized < 5.0 {
            2.0
        } else {
            5.0
        };
        coefficient * magnitude
    } else {
        scale
    }
}

pub(crate) fn rounded(value: f64, step: f64) -> f64 {
    if value.is_finite() && step.is_finite() && step > 0.0 {
        let snapped = (value / step).round() * step;
        let decimal_places = (-step.log10().floor()).max(0.0);
        let decimal_scale = 10.0_f64.powf(decimal_places);
        if decimal_scale.is_finite() {
            (snapped * decimal_scale).round() / decimal_scale
        } else {
            snapped
        }
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_completions_offer_zero_for_empty_queries_but_not_invalid_numbers() {
        for (complete, zero, representation) in [
            (
                crate::f32::completions as fn(&str) -> Vec<progred_display::Completion>,
                crate::f32::value(0.0),
                "f32",
            ),
            (crate::f64::completions, crate::f64::value(0.0), "f64"),
            (crate::u64::completions, crate::u64::value(0), "u64"),
        ] {
            for query in ["", " \t\n", "0"] {
                let offers = complete(query);
                let [offer] = offers.as_slice() else {
                    panic!("expected one {representation} zero offer for {query:?}");
                };
                assert_eq!(offer.display, "0");
                assert_eq!(offer.detail.as_deref(), Some(representation));
                assert_eq!(offer.value.literal(), Some(&zero));
            }
            for query in ["not a number", "-", ".", "1e"] {
                assert!(complete(query).is_empty(), "{representation}: {query:?}");
            }
        }
    }

    #[test]
    fn scrubbing_uses_the_active_decimal_precision() {
        let mut scrub = NumberScrub::new(100.0);

        assert_eq!(
            scrub
                .update(ScrubEvent {
                    movement_x: 4.0,
                    distance_y: 0.0,
                })
                .value,
            101.0,
        );
        assert_eq!(
            NumberScrub::new(0.5)
                .update(ScrubEvent {
                    movement_x: 4.0,
                    distance_y: 0.0,
                })
                .value,
            0.501,
        );

        let one_decimal_place = PIXELS_PER_DECADE;
        let mut scrub = NumberScrub::new(100.0);
        assert_eq!(
            scrub
                .update(ScrubEvent {
                    movement_x: 4.0,
                    distance_y: one_decimal_place,
                })
                .value,
            100.1,
        );
        assert_eq!(
            scrub
                .update(ScrubEvent {
                    movement_x: 16.0,
                    distance_y: -one_decimal_place,
                })
                .value,
            120.0,
        );
    }

    #[test]
    fn rightward_motion_never_lowers_the_displayed_value() {
        let mut scrub = NumberScrub::new(100.0);
        let first = scrub
            .update(ScrubEvent {
                movement_x: 16.0,
                distance_y: 0.0,
            })
            .value;
        let scale_changed = scrub
            .update(ScrubEvent {
                movement_x: 0.0,
                distance_y: -PIXELS_PER_DECADE,
            })
            .value;
        let moved_right = scrub
            .update(ScrubEvent {
                movement_x: 0.1,
                distance_y: -PIXELS_PER_DECADE,
            })
            .value;

        assert_eq!(first, 104.0);
        assert_eq!(scale_changed, first);
        assert!(moved_right >= scale_changed);
    }

    #[test]
    fn float_spelling_retains_the_active_decimal_precision() {
        assert_eq!(100.0.spelling(0.1), "100.0");
        assert_eq!(100.1.spelling(0.1), "100.1");
        assert_eq!(100.0.spelling(1.0), "100");
        assert_eq!(110.0.spelling(10.0), "110");
    }

    #[test]
    fn a_gesture_fixes_its_scale_from_the_starting_value() {
        assert_eq!(NumberScrub::new(0.1234838495).base, 0.001);
        assert_eq!(NumberScrub::new(123.0).base, 1.0);
        assert_eq!(NumberScrub::new(1234.0).base, 1.0);
        assert_eq!(NumberScrub::new(0_u64).base, 1.0);
        assert_eq!(NumberScrub::new(123_u64).base, 1.0);
    }

    #[test]
    fn integer_precision_stops_at_one() {
        let scrubbed = NumberScrub::new(42_u64).update(ScrubEvent {
            movement_x: 4.0,
            distance_y: 100.0,
        });

        assert_eq!(scrubbed.precision, 1.0);
        assert_eq!(scrubbed.value, 43);
    }

    #[test]
    fn precision_uses_one_two_five_steps() {
        assert_eq!(nice_precision(0.01), 0.01);
        assert_eq!(nice_precision(0.02), 0.02);
        assert_eq!(nice_precision(0.05), 0.05);
        assert_eq!(nice_precision(0.1), 0.1);
        assert_eq!(nice_precision(2.0), 2.0);
        assert_eq!(nice_precision(5.0), 5.0);
    }

    #[test]
    fn vertical_decades_spread_out_as_they_get_coarser() {
        let close = |left: f64, right: f64| (left - right).abs() < 1e-12;

        assert!(close(vertical_decades(-24.0), 1.0));
        assert!(close(vertical_decades(-60.0), 2.0));
        assert!(close(vertical_decades(-114.0), 3.0));
        assert!(close(vertical_decades(60.0), -2.0));
    }

    #[test]
    fn coarse_precision_grows_horizontal_sensitivity_sublinearly() {
        let horizontal_scale = |gain: f64| gain / gain.max(1.0).cbrt();

        assert_eq!(horizontal_scale(0.01), 0.01);
        assert_eq!(horizontal_scale(1.0), 1.0);
        assert!((horizontal_scale(1_000.0) - 100.0).abs() < 1e-12);
    }
}
