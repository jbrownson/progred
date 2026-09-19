//! Final-encoded trees. Consumers choose what to retain; source links are not GID fields.
use super::{Definitions, Library, absent, layout, name, presentation};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, SourceOrigin,
};
use gid::{CellId, Cells, Value};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[cfg(test)]
mod tests;

pub const ID: CellId = CellId::from_u128(0x0680eec8cc78f0aebe7ab8a68562a65f);
pub mod vocabulary {
    use gid::CellId;
    pub const GROUP: CellId = CellId::from_u128(0xc2a9d12a23c6bbb553682d85869481b0);
    pub const LEAF: CellId = CellId::from_u128(0xa2ee2a23ede9372c4bc71c21b6b83838);
    pub const MAP: CellId = CellId::from_u128(0x7dc1fd86e63e18ce835a5b1768f17abc);
    pub const COLLECT: CellId = CellId::from_u128(0xf070b08cda17ca0f5ec97de2ab5b2fab);
    pub const PROGRAM: CellId = CellId::from_u128(0xe4ee09094ff44e8dd415a05645f99e97);
    pub const INVALID_OUTPUT: CellId = CellId::from_u128(0x2769ad609e44a12b73cd26889ac88760);
    pub const OUTPUT_REQUIRED: CellId = CellId::from_u128(0xe350d7fb10fc3a710e7028b18d0b886b);
    pub const MAPPING: CellId = CellId::from_u128(0x4ad3bf56ebbd64849bf8d218f46d7591);
}
use layout::vocabulary::CHILDREN;
use presentation::vocabulary::VALUE;
use vocabulary::*;

/// An evaluation-local interpretation. Groups balance even on an absent/halt;
/// ordinary absent recovery retains earlier emissions, as with other effects.
pub trait Sink {
    fn begin_group(&mut self, source: Option<SourceOrigin>);
    fn end_group(&mut self);
    fn leaf(&mut self, value: Value, source: Option<SourceOrigin>);
}

struct Output<S> {
    sink: Rc<RefCell<S>>,
    maps: RefCell<Vec<::grap::PreparedCallable>>,
}

fn group<S: Sink>(
    context: &mut Context,
    expression: Expression,
    source: Option<SourceOrigin>,
    environment: &Environment,
    output: &Output<S>,
) -> Result<Value, Halt> {
    context.effect(|| output.sink.borrow_mut().begin_group(source));
    let result = children(context, expression, environment, output);
    context.effect(|| output.sink.borrow_mut().end_group());
    result
}

fn children<S: Sink>(
    context: &mut Context,
    expression: Expression,
    environment: &Environment,
    output: &Output<S>,
) -> Result<Value, Halt> {
    if let Some(elements) = context.elements(expression).map(<[_]>::to_vec) {
        for child in elements {
            context.burn()?;
            let result = if context.elements(child).is_some() {
                group(
                    context,
                    child,
                    context.source_origin(child),
                    environment,
                    output,
                )?
            } else {
                context.eval(child, environment)?
            };
            if absent::is_absent(&result) {
                return Ok(result);
            }
        }
        Ok(Value::record([]))
    } else {
        context.eval(expression, environment)
    }
}

fn functions<S: Sink + 'static>(sink: Rc<RefCell<S>>) -> ForeignFunctions {
    let output = Rc::new(Output {
        sink,
        maps: RefCell::default(),
    });
    [GROUP, LEAF, MAP]
        .into_iter()
        .fold(ForeignFunctions::default(), |functions, function| {
            let output = output.clone();
            functions.register(
                function,
                ForeignFunction::new(move |context, call, environment| {
                    let field = if function == LEAF { VALUE } else { CHILDREN };
                    let Some(expression) = context.field(call, field) else {
                        return Ok(context.missing_argument(field));
                    };
                    match function {
                        GROUP => group(
                            context,
                            expression,
                            context.source_origin(call),
                            environment,
                            &output,
                        ),
                        MAP => {
                            let Some(map) = context.field(call, MAPPING) else {
                                return Ok(context.missing_argument(MAPPING));
                            };
                            let map = context.prepare_callable(map, environment)?;
                            output.maps.borrow_mut().push(map);
                            let result = children(context, expression, environment, &output);
                            output.maps.borrow_mut().pop();
                            result
                        }
                        LEAF => {
                            let source = context.source_origin(call);
                            let mut value = context.eval_runtime(expression, environment)?;
                            let maps = output
                                .maps
                                .borrow()
                                .iter()
                                .rev()
                                .cloned()
                                .collect::<Vec<_>>();
                            for map in maps {
                                if value.is_absent() {
                                    return Ok(value.into_value());
                                }
                                value = context.call_prepared_runtime(&map, [(VALUE, value)])?;
                            }
                            if value.is_absent() {
                                return Ok(value.into_value());
                            }
                            Ok(context.effect(|| {
                                output.sink.borrow_mut().leaf(value.into_value(), source);
                                Value::record([])
                            }))
                        }
                        _ => unreachable!(),
                    }
                })
                .tracked(),
            )
        })
}

/// Install a caller-owned interpretation only for the supplied computation.
pub fn interpret<S: Sink + 'static, T>(
    context: &mut Context,
    sink: Rc<RefCell<S>>,
    run: impl FnOnce(&mut Context) -> T,
) -> T {
    context.with_foreign_functions(functions(sink), run)
}

#[derive(Clone, PartialEq)]
pub(crate) struct Built {
    pub items: Value,
    pub root: Rc<Node>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Node {
    pub source: Option<SourceOrigin>,
    // None is a leaf, even if its payload is a list; Some(empty) is an empty group.
    pub children: Option<BTreeMap<gid::Position, Node>>,
}

impl Node {
    pub fn at(&self, path: &[gid::Position]) -> Option<&Self> {
        path.iter()
            .try_fold(self, |node, position| node.children.as_ref()?.get(position))
    }
}

#[derive(Default)]
struct Collector {
    items: Vec<Built>,
    parents: Vec<(Option<SourceOrigin>, Vec<Built>)>,
}

impl Sink for Collector {
    fn begin_group(&mut self, source: Option<SourceOrigin>) {
        self.parents.push((source, std::mem::take(&mut self.items)));
    }
    fn end_group(&mut self) {
        let (source, parent) = self.parents.pop().expect("balanced group interpretation");
        let children = std::mem::replace(&mut self.items, parent);
        let items = Value::list(children.iter().map(|child| child.items.clone()));
        let children = items
            .as_list()
            .unwrap()
            .keys()
            .cloned()
            .zip(
                children
                    .into_iter()
                    .map(|child| Rc::unwrap_or_clone(child.root)),
            )
            .collect();
        self.items.push(Built {
            items,
            root: Rc::new(Node {
                source,
                children: Some(children),
            }),
        });
    }
    fn leaf(&mut self, value: Value, source: Option<SourceOrigin>) {
        self.items.push(Built {
            items: value,
            root: Rc::new(Node {
                source,
                children: None,
            }),
        });
    }
}

fn collect(
    context: &mut Context,
    program: &::grap::PreparedCallable,
) -> Result<Result<Built, Value>, Halt> {
    let sink = Rc::new(RefCell::new(Collector::default()));
    let result = interpret(context, sink.clone(), |context| {
        context.call_prepared(program, [])
    })?;
    if absent::is_absent(&result) {
        Ok(Err(result))
    } else {
        let mut sink = sink.borrow_mut();
        Ok(if sink.items.len() == 1 {
            Ok(sink.items.pop().unwrap())
        } else {
            Err(absent::with_reason(INVALID_OUTPUT))
        })
    }
}

fn evaluate(
    program: &Value,
    host: &dyn ::grap::Host,
    fuel: usize,
) -> (::grap::Evaluation, Option<Result<Built, Value>>) {
    let built = RefCell::new(None);
    let emit = |_, context: &mut Context<'_>, call, environment: &Environment| {
        let Some(expression) = context.field(call, PROGRAM) else {
            return Ok(context.missing_argument(PROGRAM));
        };
        let callable = context.prepare_callable(expression, environment)?;
        let result = collect(context, &callable)?;
        let value = result
            .as_ref()
            .map(|built| built.items.clone())
            .unwrap_or_else(Clone::clone);
        built.replace(Some(result));
        Ok(value)
    };
    let expression = ::grap::call(COLLECT.into(), [(PROGRAM, program.clone())]);
    let evaluation = ::grap::evaluate_scoped(
        &expression,
        host,
        &::grap::ForeignOverlay::new(&[COLLECT], &emit).tracked(),
        fuel,
    );
    (evaluation, built.into_inner())
}

pub(crate) fn build(program: &Value, host: &dyn ::grap::Host, fuel: usize) -> Result<Built, Value> {
    let (evaluation, built) = evaluate(program, host, fuel);
    if evaluation.completed {
        built.unwrap_or(Err(evaluation.result))
    } else {
        Err(evaluation.result)
    }
}

pub(crate) fn prepared(
    computations: &crate::computations::Computations,
    root: &crate::workspace::Root,
    path: &[gid::Step],
    program: Value,
    fuel: usize,
) -> Rc<Result<Built, Value>> {
    struct Prepared {
        input: incremental::Input<(Value, usize)>,
        result: incremental::Memo<Result<Built, Value>>,
    }
    let prepared = computations.at(root, path, || {
        let input = computations.runtime.input((program.clone(), fuel));
        let result = computations.runtime.memo({
            let input = input.clone();
            let definitions = computations.definitions.clone();
            move |read| {
                let input = input.read(read);
                let mut built = None;
                // The returned Built contains every emission; replaying those
                // effects would only rebuild the same immutable output.
                let evaluation = ::grap::memo::with_recorded_effects(&definitions, read, |host| {
                    let (evaluation, output) = evaluate(&input.0, host, input.1);
                    built = output;
                    evaluation
                });
                Ok(if evaluation.completed {
                    built.unwrap_or(Err(evaluation.result))
                } else {
                    Err(evaluation.result)
                })
            }
        });
        Prepared { input, result }
    });
    prepared.input.set((program, fuel));
    computations
        .runtime
        .read(&prepared.result)
        .unwrap_or_else(|error| Rc::new(Err(::grap::memo::failure(error))))
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, label) in [
        (ID, "Trees"),
        (GROUP, "tree group"),
        (LEAF, "tree leaf"),
        (MAP, "map tree leaves"),
        (COLLECT, "collect tree"),
        (PROGRAM, "tree program"),
        (MAPPING, "mapping"),
        (INVALID_OUTPUT, "tree requires one root"),
        (OUTPUT_REQUIRED, "tree output required"),
    ] {
        cells.set_value(cell, name::record(label, []));
    }
    let functions = [GROUP, LEAF, MAP]
        .into_iter()
        .fold(ForeignFunctions::default(), |functions, cell| {
            functions.register(
                cell,
                ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))).tracked(),
            )
        })
        .register(
            COLLECT,
            ForeignFunction::new(|context, call, environment| {
                let Some(program) = context.field(call, PROGRAM) else {
                    return Ok(context.missing_argument(PROGRAM));
                };
                let program = context.prepare_callable(program, environment)?;
                Ok(match collect(context, &program)? {
                    Ok(built) => built.items,
                    Err(error) => error,
                })
            })
            .tracked(),
        );
    Library::new(
        Definitions::from_parts(cells, functions),
        crate::display::partial(|_| None),
    )
}
