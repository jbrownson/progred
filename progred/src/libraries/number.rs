//! What the number representations share: how each is stored, read, spelled
//! and drawn, the typed reading of a number, and scrub editing. Their
//! arithmetic is Grap's, in [`crate::libraries::grap::numbers`].

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::selectable;
use crate::libraries::representation::tagged;
use crate::libraries::{Library, name};
use ::grap::RuntimeValue;
use gid::{CellId, Value};
use std::fmt::Display;
use std::rc::Rc;

pub(crate) mod scrub;
use scrub::{ScrubEvent, ScrubUpdate, on_scrub};

const PIXELS_PER_STEP: f64 = 4.0;
const PIXELS_PER_DECADE: f64 = 24.0;
const DECADE_STRETCH: f64 = 1.5;

/// One number representation: its name and tag, and how it's encoded,
/// carried at runtime, and read back.
#[derive(Clone, Copy)]
pub(crate) struct Representation<N> {
    pub name: &'static str,
    pub tag: CellId,
    pub encode: fn(N) -> Value,
    pub runtime: fn(N) -> RuntimeValue,
    pub read: fn(&RuntimeValue) -> Option<N>,
}

impl<N: Scrubbable + std::str::FromStr> Representation<N> {
    /// The library of this representation, `id`: its tag's name, its numbers
    /// drawn, and what's typed read as one.
    pub fn library<W: Host>(self, id: CellId) -> Library<W> {
        name::named_library(
            id,
            self.name,
            [(self.tag, self.name)],
            ::grap::ForeignFunctions::default(),
        )
        .with_projection(crate::display::runtime_partial(move |input| {
            self.display(input)
        }))
        .with_completions(move |request| {
            (request.scope == crate::display::CompletionScope::Literal
                && request.kind == crate::display::CompletionKind::Value)
                .then(|| self.completions(request.query))
        })
    }

    /// A number reads as its spelling, then any other fields, including one
    /// being added.
    pub fn display<W: Host>(
        self,
        input: &ProjectionInput<'_, W, RuntimeValue>,
    ) -> Option<Layout<W>> {
        let original = input.value?;
        let number = (self.read)(original)?;
        let encode = self.encode;
        let digits = crate::display::line_edit(crate::display::LineEdit {
            text: number.to_string(),
            update: Rc::new(move |spelling: &str, current: Option<&Value>| {
                edit(spelling, current, encode)
            }),
            prefix: String::new(),
            suffix: String::new(),
            family: crate::display::TextFamily::SystemUi,
            placeholder: None,
        });
        let line = tagged(input, digits, self.tag);
        let site = input.site();
        let line = selectable(line, &site);
        let line = if number.scrubbable() {
            let original = original.clone();
            // Outlined where it drags: the whole number, its tag too.
            on_scrub(
                crate::display::drag_outlined(line),
                W::Hover::from(site.hover()),
                Rc::new(move || {
                    let original = original.to_value();
                    let mut scrub = NumberScrub::new(number);
                    Box::new(move |event| {
                        let scrubbed = scrub.update(event);
                        ScrubUpdate {
                            value: super::keeping_fields(Some(&original), encode(scrubbed.value)),
                            spelling: scrubbed.value.spelling(scrubbed.precision),
                        }
                    })
                }),
            )
        } else {
            line
        };
        Some(crate::display::structure::Fields::default().after(
            input,
            |key| key == self.tag || key == name::vocabulary::NAME,
            name::with_name(input, line),
        ))
    }

    pub fn completions(self, query: &str) -> Vec<crate::display::Completion> {
        query
            .trim()
            .parse::<N>()
            .ok()
            .map(|number| {
                crate::libraries::completion::select(number.to_string(), (self.encode)(number))
                    .with_aliases([query])
                    .with_detail(self.tag)
            })
            .into_iter()
            .collect()
    }
}

pub(crate) trait Scrubbable: Copy + Display + PartialOrd + 'static {
    fn magnitude(self) -> f64;
    fn minimum_precision() -> f64;
    fn scrubbable(self) -> bool;
    fn from_offset(start: Self, offset: f64, precision: f64) -> Self;
    fn spelling(self, precision: f64) -> String;
}

pub(crate) fn edit<N: std::str::FromStr>(
    spelling: &str,
    current: Option<&Value>,
    encode: fn(N) -> Value,
) -> Option<Value> {
    let value = encode(spelling.trim().parse().ok()?);
    Some(super::keeping_fields(current, value))
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

/// The step a scrub starts with: the value's second significant digit, at
/// most one, so a two-digit number moves by ones.
fn initial_precision(magnitude: f64, minimum: f64) -> f64 {
    (if magnitude == 0.0 {
        0.1
    } else {
        10.0_f64
            .powf(magnitude.abs().log10().floor() - 1.0)
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
#[path = "tests/number/mod.rs"]
mod tests;
