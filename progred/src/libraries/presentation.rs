//! Projections for computed artifacts. A document may place one in a
//! pane, whose view opts into applying its declared projection. The
//! document view leaves that declaration editable. A projection function
//! receives one argument under [`vocabulary::VALUE`] and returns a value;
//! an absent result exposes its source. An explicit viewport function also
//! receives the pane's assigned logical width and height.

use crate::display::at;
use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::libraries::{Library, f64, layout, name};
use gid::Step;

mod outline;

pub const ID: gid::CellId = gid::CellId::from_u128(0xd22b834154d60b1df228f9bb4d3c13de);

pub mod vocabulary {
    use gid::CellId;

    pub const RENDER: CellId = CellId::from_u128(0x37cda4bdea0091349e305951564fbdf1);
    pub use crate::workspace::vocabulary::PROJECTION;
    pub const VIEWPORT: CellId = CellId::from_u128(0x709c987e4c2a110931d8597c4da68f69);
    /// Optional size-independent preparation of a viewport's data input.
    pub const PREPARE: CellId = CellId::from_u128(0x8b782978097343efd90f6fb61f649419);
    /// The single argument of a projection function.
    pub const VALUE: CellId = CellId::from_u128(0x84d3ba81fd2a52ea37478f4a868106f4);
    /// Ordered field references for an opt-in record outline.
    pub const OUTLINE: CellId = CellId::from_u128(0xe6c0b2058b8468428d64576d953e2d62);
    /// Occurrence step for a computed result, not a field in the source record.
    pub const RESULT: CellId = CellId::from_u128(0x11098129f74918af8a0924f94c37df4f);
}

pub fn viewport(value: &gid::Value) -> Option<(&gid::Value, &gid::Value)> {
    let fields = value.as_record()?;
    Some((
        fields.get(&vocabulary::VALUE)?,
        fields.get(&vocabulary::VIEWPORT)?,
    ))
}

/// A viewport function receives its assigned logical size, not a size
/// inferred from its output. Its result uses the ordinary layout and handlers.
pub fn viewport_display<W: Host>(
    input: &ProjectionInput<'_, W>,
    width: f64,
    height: f64,
) -> Option<Layout<W>> {
    let declaration = input.value?;
    if width <= 0.0 || height <= 0.0 {
        viewport(declaration).map(|_| crate::display::row(0.0, []))
    } else {
        viewport_runtime_output(declaration, input.env, width, height).map(result_or_value)
    }
}

/// A projection's result in place, or where it's absent, the value it
/// projects, so the absence exposes its source.
fn result_or_value<W: Host>(result: ::grap::RuntimeValue) -> Layout<W> {
    if result.is_absent() {
        crate::display::descend([Step::Key(vocabulary::VALUE)], None, None)
    } else {
        at([Step::Key(vocabulary::RESULT)], &result)
    }
}

fn viewport_runtime_output(
    declaration: &gid::Value,
    env: &dyn crate::display::Env,
    width: f64,
    height: f64,
) -> Option<::grap::RuntimeValue> {
    let (value, function) = viewport(declaration)?;
    let fields = declaration.as_record()?;
    let value = match fields.get(&vocabulary::PREPARE) {
        Some(prepare) => {
            let fuel = match fields.get(&layout::vocabulary::FUEL) {
                Some(value) => ::grap::f64::to_usize(f64::read(value)?)?,
                None => ::grap::DEFAULT_FUEL,
            };
            let prepared =
                env.apply_runtime_memo(prepare, &[(vocabulary::VALUE, value.clone())], fuel);
            if prepared.is_absent() {
                return Some(prepared);
            }
            prepared
        }
        None => value.into(),
    };
    Some(env.apply_expression_runtime(
        &function.into(),
        &[
            (vocabulary::VALUE, value),
            (layout::vocabulary::WIDTH, ::grap::RuntimeValue::f64(width)),
            (
                layout::vocabulary::HEIGHT,
                ::grap::RuntimeValue::f64(height),
            ),
        ],
    ))
}

pub fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let expression = input.value?.field(vocabulary::RENDER)?;
    let evaluated = (|| {
        let fuel = ::grap::f64::to_usize(expression.field(layout::vocabulary::FUEL)?.as_f64()?)?;
        let expression = expression.field(::grap::vocabulary::EXPRESSION)?;
        Some(input.env.evaluate_runtime_memo(
            &expression,
            fuel,
            &[
                Step::Key(vocabulary::RENDER),
                Step::Key(::grap::vocabulary::EXPRESSION),
            ],
        ))
    })();
    let result = evaluated.unwrap_or_else(|| {
        input.env.evaluate_runtime_memo(
            &expression,
            ::grap::DEFAULT_FUEL,
            &[Step::Key(vocabulary::RENDER)],
        )
    });
    Some(at([Step::Key(vocabulary::RESULT)], &result))
}

thread_local! {
    /// Where document libraries are drawing right now. One steps aside where
    /// it's already drawing, so its view can show the value's ordinary
    /// projection inside itself.
    static DRAWING: std::cell::RefCell<Vec<std::rc::Rc<[Step]>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// How deeply document libraries' drawings may nest before the loaded
/// libraries take over, so a view that keeps drawing itself still ends.
const NESTING: usize = 64;

struct Drawing;

impl Drawing {
    fn at(occurrence: std::rc::Rc<[Step]>) -> Self {
        DRAWING.with(|drawing| drawing.borrow_mut().push(occurrence));
        Self
    }
}

impl Drop for Drawing {
    fn drop(&mut self) {
        DRAWING.with(|drawing| drawing.borrow_mut().pop());
    }
}

/// The libraries a document declares at its root, tried in order on a record
/// carrying a key one owns, once every loaded library declines it. A
/// projection that emits a layout draws the record in place, so whatever it
/// descends into stays editable; one that returns a value draws that value
/// with the loaded libraries; an absent passes the record on.
pub(crate) fn document_libraries<W: Host>(loaded: Partial<W>) -> Partial<W> {
    crate::display::runtime_partial(move |input| {
        let value = input.value?;
        let occurrence: std::rc::Rc<[Step]> = input.env.occurrence().into();
        DRAWING
            .with(|drawing| {
                let drawing = drawing.borrow();
                drawing.len() < NESTING && !drawing.contains(&occurrence)
            })
            .then_some(())?;
        input
            .env
            .libraries()
            .into_iter()
            .filter(|library| library.keys.iter().any(|key| value.field(*key).is_some()))
            .find_map(|library| {
                let projection = library.projection;
                let (evaluation, layout) = layout::scope::run(
                    || input.site(),
                    |scope| {
                        input.env.apply_runtime_scoped(
                            &projection,
                            &[(vocabulary::VALUE, value.clone())],
                            Some(scope),
                        )
                    },
                );
                let layout = match layout {
                    // The view chose what to show; only a field being added
                    // follows it.
                    Some(layout) => {
                        crate::display::structure::with_extra_fields(input, |_| true, layout)
                    }
                    None => {
                        (evaluation.completed && !evaluation.result.is_absent()).then(|| {
                            crate::display::at_with_projection(
                                [Step::Key(vocabulary::RESULT)],
                                evaluation.result,
                                Some(loaded.clone()),
                                Some(loaded.clone()),
                            )
                        })?
                    }
                };
                let occurrence = occurrence.clone();
                Some(Layout::new(move |builder| {
                    let _drawing = Drawing::at(occurrence.clone());
                    layout.run(builder)
                }))
            })
    })
}

/// Opt-in presentation of a declaration; not part of the library's
/// ordinary authoring projection.
pub fn projected_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let value = input.value?.field(vocabulary::VALUE)?;
    let function = input.value?.field(vocabulary::PROJECTION)?;
    Some(result_or_value(input.env.apply_expression_runtime(
        &function,
        &[(vocabulary::VALUE, value)],
    )))
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "presentation",
        [
            (vocabulary::RENDER, "render"),
            (vocabulary::PROJECTION, "projection"),
            (vocabulary::VIEWPORT, "viewport"),
            (vocabulary::PREPARE, "prepare"),
            (vocabulary::VALUE, "value"),
            (vocabulary::OUTLINE, "outline"),
            (vocabulary::RESULT, "result"),
        ],
        ::grap::ForeignFunctions::default(),
    )
    .with_projection(crate::display::compose_partials([
        crate::display::runtime_partial(display),
        crate::display::partial(outline::display),
    ]))
}

#[cfg(test)]
pub(crate) mod tests;
