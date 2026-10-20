//! Grap's arithmetic over the number representations: each one's operations
//! as primitives Grap runs natively, named with their representation's tag
//! as a subscript, read infix, and typed after a number; the argument labels
//! of binary and unary operations, which list concatenation takes too; and
//! conversions between them. The numbers themselves belong to f64, f32 and
//! u64, which know nothing of Grap.

use crate::display::host::{Host, Layout, Partial};
use crate::libraries::number::{Representation, Scrubbable};
use crate::libraries::representation::{Precedence, infix_display};
use crate::libraries::u64 as u64_library;
use crate::libraries::{f32 as f32_library, f64 as f64_library};
use crate::libraries::{name, text};
use ::grap::arguments::Parse;
use ::grap::primitive::{Binary, Number, Primitive, Unary};
use ::grap::{Context, ForeignFunction, ForeignFunctions, RuntimeValue};
use gid::{CellId, Value};
use puri::handler::HasHandler;
use std::rc::Rc;

pub mod vocabulary {
    use gid::CellId;

    pub const LEFT: CellId = CellId::from_u128(0x764f6afe17ba14e81f5ab61204be0bec);
    pub const RIGHT: CellId = CellId::from_u128(0x4f53ff25390f58472d31a6142644dec2);
    pub const OPERAND: CellId = CellId::from_u128(0x50a20d15e4ae56be51b882de9d58c676);
}

/// f64's operations and the reasons they decline.
pub mod f64 {
    use gid::CellId;

    pub const SUM: CellId = CellId::from_u128(0x201af445eb7e2c270bb5ead10b781fc1);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd6f384c439d9d69996d545df422efd79);
    pub const SUBTRACT: CellId = CellId::from_u128(0x08d1ebc7fd4ce62efec9671f73e9b645);
    pub const DIVIDE: CellId = CellId::from_u128(0xb08dd4c44eeea43ab3c7593ddeedd742);
    pub const SIN: CellId = CellId::from_u128(0x9f62e0f56d92be74f6ae36378d889262);
    pub const COS: CellId = CellId::from_u128(0xfb05b6c9b1565ca4add3732e8ddbe2b4);
    pub const LESS: CellId = CellId::from_u128(0xed44dbf5b4cdf5c952e1ef00f219b655);
    pub const EQUAL: CellId = CellId::from_u128(0x22ab9aa3e7ce4f4f79a7039e1cc23773);
    pub const FLOOR: CellId = CellId::from_u128(0xd007814c5f6a6c38b025605b399473d4);
    pub const MIN: CellId = CellId::from_u128(0x6daed3f67cff3da30767201e5a895f1a);
    pub const MAX: CellId = CellId::from_u128(0x9dfc13c9c7e059da42ea74ef63897ce6);
    pub const CEIL: CellId = CellId::from_u128(0xe1ddb3a620b61117d3873c163f603488);
    pub const HYPOT: CellId = CellId::from_u128(0xfe79fe9c7be4542249fdf604ddf5f5ea);
    pub const IS_FINITE: CellId = CellId::from_u128(0x1ff1fcec05b669eba2314ad52fe8e8f1);
    pub const LERP: CellId = CellId::from_u128(0x432ad7a31ef129e353e251419e690ca1);
    pub const START: CellId = CellId::from_u128(0x4b4fb6349d2fd798e7aafca85a2deca8);
    pub const END: CellId = CellId::from_u128(0x3a816b0af0160bc77ba1948b122b3f29);
    pub const AMOUNT: CellId = CellId::from_u128(0x55e66fc5eb91699cf12833fb0a15d4b6);
    pub use ::grap::f64::NOT_F64;
}

/// f32's operations and the reasons they decline.
pub mod f32 {
    use gid::CellId;

    pub const SUM: CellId = CellId::from_u128(0x257e5967d826e69d28929cf365667ae1);
    pub const SUBTRACT: CellId = CellId::from_u128(0x5cd7408dab067d8b92c7a1bd9cda7b05);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd4df1fe63a95cda53c19ad7220054c24);
    pub const DIVIDE: CellId = CellId::from_u128(0xfbf1ebc4eeeea21f86af583f6ad85e71);
    pub const LESS: CellId = CellId::from_u128(0x3dde12d1f97ccca65c579ae2703bb07a);
    pub const EQUAL: CellId = CellId::from_u128(0xe0637a60944f8dd9afd2fb4c28214dc3);
    pub use ::grap::f32::NOT_F32;
    pub const F32_FROM_F64: CellId = CellId::from_u128(0x73bd2225d2091ba893a5570e8e9bacff);
}

/// u64's operations and the reasons they decline.
pub mod u64 {
    use gid::CellId;

    pub const SUM: CellId = CellId::from_u128(0x2b63a57fb5065d7ae992241b3018fcb5);
    pub const SUBTRACT: CellId = CellId::from_u128(0x74203a2808e4216a6eff871dd39ab313);
    pub const MULTIPLY: CellId = CellId::from_u128(0xeb9073084d92e0ecb2253ff70a27c03e);
    pub const DIVIDE: CellId = CellId::from_u128(0x610171b11dd2d2844119b7f9bc7da17c);
    pub const LESS: CellId = CellId::from_u128(0x425af3c983c925e2eb8becf4389aac58);
    pub const EQUAL: CellId = CellId::from_u128(0xc3931b2321d6c783fee133acaa929738);
    pub use ::grap::u64::NOT_U64;
    pub const OVERFLOW: CellId = CellId::from_u128(0x184f82a04d9cb7609238b2dd05ad5cd0);
    pub const DIVISION_BY_ZERO: CellId = CellId::from_u128(0x7eef118f55e12f09c8f7324c0fdd765d);
}

/// What arithmetic adds to the Grap library: the names of its cells, its
/// functions, and its projection, which goes before Grap's calls'.
pub(super) fn contribution<W: Host>() -> (Vec<(CellId, Value)>, ForeignFunctions, Partial<W>) {
    let mut parts = [f64_parts(), f32_parts(), u64_parts()];
    let names = [
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
        (vocabulary::OPERAND, "operand"),
    ]
    .map(|(cell, name)| (cell, text::value(name)))
    .into_iter()
    .chain(
        parts
            .iter_mut()
            .flat_map(|parts| std::mem::take(&mut parts.names)),
    )
    .collect();
    let functions = ForeignFunctions::merge_all(
        parts
            .iter_mut()
            .map(|parts| std::mem::take(&mut parts.functions)),
    );
    let [f64, f32, u64] = parts;
    let projection = crate::display::compose_partials([
        f64.projection(f64_arithmetic()),
        f32.projection(f32_arithmetic()),
        u64.projection(u64_arithmetic()),
    ]);
    (names, functions, projection)
}

/// An operation's name: its spelling, then its representation's `tag` as a
/// subscript, telling apart the operators each representation has.
fn operation_name(spelling: &str, tag: CellId) -> Value {
    name::formatted([text::value(spelling), name::subscript(tag.into())])
}

fn f64_arithmetic() -> Arithmetic<::core::primitive::f64> {
    Arithmetic {
        representation: f64_library::representation(),
        not: f64::NOT_F64,
    }
}

fn f32_arithmetic() -> Arithmetic<::core::primitive::f32> {
    Arithmetic {
        representation: f32_library::representation(),
        not: f32::NOT_F32,
    }
}

fn u64_arithmetic() -> Arithmetic<::core::primitive::u64> {
    Arithmetic {
        representation: u64_library::representation(),
        not: u64::NOT_U64,
    }
}

fn f64_parts() -> Parts {
    use ::core::primitive::f64 as N;
    use Operation::{Arithmetic, Comparison, Predicate, Unary};
    let mut parts = f64_arithmetic().parts([
        (f64::SUM, "+", Arithmetic(|left, right| left + right)),
        (f64::MULTIPLY, "*", Arithmetic(|left, right| left * right)),
        (f64::SUBTRACT, "-", Arithmetic(|left, right| left - right)),
        (f64::DIVIDE, "/", Arithmetic(|left, right| left / right)),
        (f64::MIN, "min", Arithmetic(N::min)),
        (f64::MAX, "max", Arithmetic(N::max)),
        (f64::HYPOT, "hypot", Arithmetic(N::hypot)),
        (f64::LESS, "<", Comparison(|left, right| left < right)),
        (f64::EQUAL, "==", Comparison(|left, right| left == right)),
        (f64::SIN, "sin", Unary(N::sin)),
        (f64::COS, "cos", Unary(N::cos)),
        (f64::FLOOR, "floor", Unary(N::floor)),
        (f64::CEIL, "ceil", Unary(N::ceil)),
        (f64::IS_FINITE, "is finite", Predicate(N::is_finite)),
    ]);
    parts.functions = parts.functions.register(
        f64::LERP,
        ForeignFunction::with_arguments([f64::START, f64::END, f64::AMOUNT], lerp).tracked(),
    );
    parts.names.extend(
        [
            (f64::LERP, "lerp"),
            (f64::START, "start"),
            (f64::END, "end"),
            (f64::AMOUNT, "amount"),
        ]
        .map(|(cell, name)| (cell, text::value(name))),
    );
    parts.names.push((
        f64::LERP,
        operation_name("lerp", f64_library::vocabulary::F64),
    ));
    parts
}

fn lerp(
    _: &mut Context,
    (start, end, amount): (
        ::core::primitive::f64,
        ::core::primitive::f64,
        ::core::primitive::f64,
    ),
) -> RuntimeValue {
    RuntimeValue::f64(start + (end - start) * amount)
}

fn f32_parts() -> Parts {
    use Operation::{Arithmetic, Comparison};
    let mut parts = f32_arithmetic().parts([
        (f32::SUM, "+", Arithmetic(|left, right| left + right)),
        (f32::SUBTRACT, "-", Arithmetic(|left, right| left - right)),
        (f32::MULTIPLY, "*", Arithmetic(|left, right| left * right)),
        (f32::DIVIDE, "/", Arithmetic(|left, right| left / right)),
        (f32::LESS, "<", Comparison(|left, right| left < right)),
        (f32::EQUAL, "==", Comparison(|left, right| left == right)),
    ]);
    parts.functions = parts.functions.register(
        f32::F32_FROM_F64,
        conversion(
            f32_library::representation().runtime,
            |number: ::core::primitive::f64| number as ::core::primitive::f32,
        ),
    );
    parts
        .names
        .push((f32::F32_FROM_F64, text::value("f32 from f64")));
    parts
}

/// Unsigned arithmetic has no representable overflow or division by
/// zero, so those outcomes are absents rather than wrapped bits.
fn u64_parts() -> Parts {
    use ::core::primitive::u64 as N;
    use Operation::{Checked, Comparison};
    let mut parts = u64_arithmetic().parts([
        (u64::SUM, "+", Checked(N::checked_add, u64::OVERFLOW)),
        (u64::SUBTRACT, "-", Checked(N::checked_sub, u64::OVERFLOW)),
        (u64::MULTIPLY, "*", Checked(N::checked_mul, u64::OVERFLOW)),
        (
            u64::DIVIDE,
            "/",
            Checked(N::checked_div, u64::DIVISION_BY_ZERO),
        ),
        (u64::LESS, "<", Comparison(|left, right| left < right)),
        (u64::EQUAL, "==", Comparison(|left, right| left == right)),
    ]);
    parts.names.extend(
        [
            (u64::OVERFLOW, "u64 overflow"),
            (u64::DIVISION_BY_ZERO, "division by zero"),
        ]
        .map(|(cell, name)| (cell, text::value(name))),
    );
    parts
}

/// Comparisons and the arithmetic symbols read infix; named and one-operand
/// operations stay calls.
fn precedence<N>(spelling: &str, operation: &Operation<N>) -> Option<Precedence> {
    match operation {
        Operation::Comparison(_) => Some(Precedence::Comparison),
        Operation::Arithmetic(_) | Operation::Checked(..) => Precedence::of_symbol(spelling),
        Operation::Unary(..) | Operation::Predicate(..) => None,
    }
}

/// Grap's arithmetic over one representation, whose operations Grap runs
/// natively, and why one declines.
#[derive(Clone, Copy)]
struct Arithmetic<N> {
    representation: Representation<N>,
    not: CellId,
}

#[derive(Clone, Copy)]
enum Operation<N> {
    Arithmetic(fn(N, N) -> N),
    /// Fails with the given reason instead of producing a result.
    Checked(fn(N, N) -> Option<N>, CellId),
    Comparison(fn(N, N) -> bool),
    Unary(fn(N) -> N),
    Predicate(fn(N) -> bool),
}

/// One representation's operations: their names, functions, and infix
/// forms.
struct Parts {
    names: Vec<(CellId, Value)>,
    functions: ForeignFunctions,
    /// The calls that read infix, and how tightly each binds.
    infix: Vec<(CellId, Precedence)>,
    /// The infix operators by spelling, for typing after a number.
    symbols: Vec<(&'static str, CellId)>,
}

impl Parts {
    /// The calls drawn infix, and a number that takes its operators as
    /// typing.
    fn projection<W: Host, N: Scrubbable + std::str::FromStr>(
        self,
        arithmetic: Arithmetic<N>,
    ) -> Partial<W> {
        let infix: Rc<[(CellId, Precedence)]> = self.infix.into();
        let symbols: Rc<[(&'static str, CellId)]> = self.symbols.into();
        crate::display::compose_partials([
            crate::display::runtime_partial(move |input| infix_display(&infix, input)),
            crate::display::runtime_partial(move |input| {
                let layout = arithmetic.representation.display(input)?;
                Some(if input.writable && input.selection.is_some() {
                    typed_operators::<_, N>(layout, symbols.clone())
                } else {
                    layout
                })
            }),
        ])
    }
}

impl<N: Scrubbable + std::str::FromStr + Number> Arithmetic<N> {
    fn parts(
        self,
        operations: impl IntoIterator<Item = (CellId, &'static str, Operation<N>)>,
    ) -> Parts {
        let name = self.representation.name;
        let tag = self.representation.tag;
        operations.into_iter().fold(
            Parts {
                names: vec![(self.not, text::value(format!("not {name}")))],
                functions: ForeignFunctions::default(),
                infix: Vec::new(),
                symbols: Vec::new(),
            },
            |mut parts, (cell, spelling, operation)| {
                if let Some(precedence) = precedence(spelling, &operation) {
                    parts.infix.push((cell, precedence));
                    parts.symbols.push((spelling, cell));
                }
                parts.names.push((cell, operation_name(spelling, tag)));
                parts.functions = parts.functions.register(cell, self.operation(operation));
                parts
            },
        )
    }

    /// `operation` as the primitive Grap runs.
    fn operation(self, operation: Operation<N>) -> ForeignFunction {
        let binary = |operation| Primitive::Binary {
            operation: N::binary(operation),
            left: vocabulary::LEFT,
            right: vocabulary::RIGHT,
        };
        let unary = |operation| Primitive::Unary {
            operation: N::unary(operation),
            operand: vocabulary::OPERAND,
        };
        match operation {
            Operation::Arithmetic(apply) => binary(Binary::Arithmetic(apply)),
            Operation::Checked(apply, failure) => binary(Binary::Checked(apply, failure)),
            Operation::Comparison(apply) => binary(Binary::Comparison(apply)),
            Operation::Unary(apply) => unary(Unary::Arithmetic(apply)),
            Operation::Predicate(apply) => unary(Unary::Predicate(apply)),
        }
        .function()
    }
}

/// To a number made by `runtime` from another, its operand, by `convert`.
fn conversion<From: Parse + 'static, N: 'static>(
    runtime: fn(N) -> RuntimeValue,
    convert: fn(From) -> N,
) -> ForeignFunction {
    ForeignFunction::with_arguments([vocabulary::OPERAND], move |_, (number,): (From,)| {
        runtime(convert(number))
    })
    .tracked()
}

/// A selected number takes its type's single-character operators as typing:
/// one typed at the end of a whole number makes the number its left operand,
/// with the right operand's picker open. A sign or exponent that doesn't yet
/// make a number still goes into its text.
fn typed_operators<W: Host, N: std::str::FromStr + 'static>(
    child: Layout<W>,
    symbols: Rc<[(&'static str, CellId)]>,
) -> Layout<W> {
    crate::display::widget::after(
        child,
        Rc::new(move |context| {
            let site = context.site();
            let symbols = symbols.clone();
            Box::new(move |output, _| {
                output.handler().on_key(move |world: &mut W, event| {
                    wrap_typed::<N>(&mut world.session(), event, &site, &symbols)
                });
            })
        }),
    )
}

fn wrap_typed<N: std::str::FromStr>(
    session: &mut crate::editing::Session<'_>,
    event: &ui_events::keyboard::KeyboardEvent,
    site: &crate::editing::Site,
    symbols: &[(&'static str, CellId)],
) -> bool {
    typed_operator(event, symbols).is_some_and(|operator| {
        let mut edit = site.open(session);
        let wrapped = ends_a_number::<N>(edit.selected())
            && edit.value().cloned().is_some_and(|left| {
                edit.replace(::grap::call(
                    Value::from(operator),
                    [(vocabulary::LEFT, left)],
                ))
            });
        if wrapped {
            edit.pend(&[gid::Step::Key(vocabulary::RIGHT)]);
        }
        wrapped
    })
}

/// The operator among `symbols` that an unmodified keypress types.
fn typed_operator(
    event: &ui_events::keyboard::KeyboardEvent,
    symbols: &[(&'static str, CellId)],
) -> Option<CellId> {
    let plain = event.state.is_down() && crate::modifiers::at_most_shift(&event.modifiers);
    match &event.key {
        ui_events::keyboard::Key::Character(typed) if plain => symbols
            .iter()
            .find(|(spelling, _)| *spelling == typed.as_str())
            .map(|&(_, operator)| operator),
        _ => None,
    }
}

/// Whether a typed operator follows the whole number: the number is
/// selected, or its line is being typed with the caret after all of it.
fn ends_a_number<N: std::str::FromStr>(selected: crate::editing::Selected<'_>) -> bool {
    match selected {
        crate::editing::Selected::Elsewhere => false,
        crate::editing::Selected::Here => true,
        crate::editing::Selected::Typing(line) => {
            let (anchor, focus) = line.selection_offsets();
            anchor == focus && focus == line.text().len() && line.text().trim().parse::<N>().is_ok()
        }
    }
}

#[cfg(test)]
mod tests;
