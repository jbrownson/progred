//! Calls: closures, cell and foreign function calls, and prepared callables.

use crate::environment::{CellIndex, CellIndices, Environment, binding_index};
use crate::eval::{missing_cell, unbox};
use crate::foreign::{
    ForeignFunction, ForeignFunctions, ForeignImplementation, OverlayCall, Prepare, Stage,
};
use crate::lower::{OriginId, OriginNode, OriginRoot, runtime_call};
use crate::runtime::{Closure, RuntimeValue, RuntimeValueKind};
use crate::{
    Context, Definition, Expression, Halt, absent, arguments, isa, primitive, source, vocabulary,
};
use gid::{CellId, Value};
use std::cell::RefCell;
use std::rc::Rc;
/// An evaluated callable retained in its compact runtime form. This
/// lets a controlling FFI invoke the same Grap function repeatedly
/// without reifying and reparsing a closure Value between calls.
#[derive(Clone)]
pub struct PreparedCallable {
    pub(crate) value: RuntimeValue,
    environment: Environment,
}

/// A call node's memory of its last callee: the parameters slice (held
/// so its identity stays valid) and the argument expression this call
/// supplies for each parameter, in declaration order.
pub(crate) struct CallPlan {
    params: Rc<[CellId]>,
    arguments: Rc<[arguments::Argument]>,
    /// The parameters' indices in `table`, the callee environment's, so a
    /// call binds them without looking each one up.
    table: CellIndices,
    indices: Rc<[CellIndex]>,
}

pub(crate) struct PreparedCallTarget {
    kind: PreparedCallTargetKind,
    plan: RefCell<Option<CallPlan>>,
    stages: RefCell<Option<(Prepare, Stage)>>,
}

enum PreparedCallTargetKind {
    Foreign(ForeignFunction),
    /// A primitive with this call site's operands, run directly.
    Primitive(primitive::Primitive, primitive::Operands),
    Value(Expression),
    Missing,
}

#[derive(Clone)]
pub(crate) enum ResolvedForeign {
    Function(ForeignFunction),
    Overlay(CellId),
}

impl<'a> Context<'a> {
    pub fn closure(
        &self,
        params: impl IntoIterator<Item = CellId>,
        body: Expression,
        environment: &Environment,
    ) -> RuntimeValue {
        let params: Rc<[CellId]> = params.into_iter().collect();
        let fields = [
            (isa::ISA, Value::from(vocabulary::kind::LAMBDA)),
            (
                vocabulary::PARAMS,
                Value::list(params.iter().copied().map(Value::from)),
            ),
            (vocabulary::BODY, body.value().clone()),
        ]
        .into_iter()
        .collect();
        RuntimeValue::new(RuntimeValueKind::Closure(Closure {
            fields,
            params,
            body,
            environment: environment.clone(),
        }))
    }

    pub fn closure_value(
        &mut self,
        params: impl IntoIterator<Item = CellId>,
        body: Value,
        environment: &Environment,
    ) -> RuntimeValue {
        let body = self.lower_unattributed_source(&body);
        self.closure(params, body, environment)
    }

    pub(crate) fn call_cell(
        &mut self,
        cell: CellId,
        call: &Expression,
        environment: &Environment,
        cache: Option<&RefCell<Option<Rc<PreparedCallTarget>>>>,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        let cached = cache.and_then(|cache| cache.borrow().clone());
        let target = match cached {
            Some(target) => target,
            None => {
                let kind = match self.host.resolve(cell) {
                    Some(Definition::Foreign(definition)) => {
                        match &definition.implementation.primitive {
                            Some(primitive) => PreparedCallTargetKind::Primitive(
                                **primitive,
                                primitive.operands(call),
                            ),
                            None => {
                                PreparedCallTargetKind::Foreign(definition.implementation.clone())
                            }
                        }
                    }
                    Some(Definition::Value(value)) => {
                        PreparedCallTargetKind::Value(self.lowered_cell(cell, &value))
                    }
                    None => PreparedCallTargetKind::Missing,
                };
                let target = Rc::new(PreparedCallTarget {
                    kind,
                    plan: RefCell::new(None),
                    stages: RefCell::new(None),
                });
                if let Some(cache) = cache {
                    *cache.borrow_mut() = Some(target.clone());
                }
                target
            }
        };
        self.checked_call(|context| match &target.kind {
            PreparedCallTargetKind::Foreign(function) => context.call_foreign_staged(
                &ResolvedForeign::Function(function.clone()),
                call,
                environment,
                Some(&target.stages),
            ),
            PreparedCallTargetKind::Value(expression) => {
                let callable = context.eval_definition(cell, expression.clone(), environment)?;
                if callable.absent_reason().is_some() {
                    Ok(callable)
                } else {
                    context
                        .try_call_callable(
                            callable.clone(),
                            call,
                            environment,
                            &target.plan,
                            &target.stages,
                        )
                        .unwrap_or_else(|| Ok(not_callable(&callable)))
                }
            }
            PreparedCallTargetKind::Primitive(primitive, operands) => {
                primitive.run(context, call, operands, environment)
            }
            PreparedCallTargetKind::Missing => Ok(missing_cell(cell)),
        })
    }

    pub(crate) fn try_call_callable(
        &mut self,
        callable: RuntimeValue,
        call: &Expression,
        environment: &Environment,
        plan: &RefCell<Option<CallPlan>>,
        stages: &RefCell<Option<(Prepare, Stage)>>,
    ) -> Option<Result<RuntimeValue, Halt>> {
        match &callable.0 {
            RuntimeValueKind::Closure(closure) => {
                Some(self.eval_grap_call(closure.clone(), call, environment, plan))
            }
            RuntimeValueKind::Data(_)
            | RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_) => match self.runtime_closure(&callable) {
                Some(closure) => Some(self.eval_grap_call(closure, call, environment, plan)),
                None => callee_cell(&callable).map(|cell| match self.foreign_target_cell(cell) {
                    Some(foreign) => {
                        self.call_foreign_staged(&foreign, call, environment, Some(stages))
                    }
                    None => self.call_cell(cell, call, environment, None),
                }),
            },
            RuntimeValueKind::List(_) => None,
        }
    }

    pub(crate) fn foreign_target_cell(&self, cell: CellId) -> Option<ResolvedForeign> {
        self.scoped_foreign_target(cell).or_else(|| {
            self.overlay
                .is_some_and(|overlay| overlay.handles(cell))
                .then_some(ResolvedForeign::Overlay(cell))
        })
    }

    fn scoped_foreign_target(&self, cell: CellId) -> Option<ResolvedForeign> {
        self.foreign_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(cell))
            .map(|function| ResolvedForeign::Function(function.clone()))
    }

    fn call_foreign(
        &mut self,
        foreign: &ResolvedForeign,
        call: &Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.call_foreign_staged(foreign, call, environment, None)
    }

    /// Call `foreign` with evaluated arguments, which it receives as the
    /// fields of a call to `callable`.
    fn apply_foreign(
        &mut self,
        foreign: &ResolvedForeign,
        callable: RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let call = self.lower_runtime_code(&RuntimeValue::record(
            [
                (
                    isa::ISA,
                    RuntimeValue::from(Value::from(vocabulary::kind::CALL)),
                ),
                (vocabulary::FUNCTION, callable),
            ]
            .into_iter()
            .chain(arguments),
        ));
        self.call_foreign(foreign, &call, environment)
    }

    fn call_foreign_staged(
        &mut self,
        foreign: &ResolvedForeign,
        call: &Expression,
        environment: &Environment,
        stages: Option<&RefCell<Option<(Prepare, Stage)>>>,
    ) -> Result<RuntimeValue, Halt> {
        let tracked = match foreign {
            ResolvedForeign::Function(function) => function.tracked,
            ResolvedForeign::Overlay(_) => self.overlay.is_some_and(|overlay| overlay.tracked),
        };
        if !tracked {
            self.host.untracked();
        }
        let value = match foreign {
            ResolvedForeign::Function(function) => match &function.implementation {
                ForeignImplementation::Direct(call_direct) => call_direct(self, call, environment),
                ForeignImplementation::Staged(prepare) => {
                    let cached = stages.and_then(|slot| {
                        slot.borrow().as_ref().and_then(|(cached_prepare, stage)| {
                            Rc::ptr_eq(cached_prepare, prepare).then(|| stage.clone())
                        })
                    });
                    let stage = match cached {
                        Some(stage) => stage,
                        None => {
                            let stage = prepare(call);
                            if let Some(slot) = stages {
                                *slot.borrow_mut() = Some((prepare.clone(), stage.clone()));
                            }
                            stage
                        }
                    };
                    stage(self, environment)
                }
            },
            ResolvedForeign::Overlay(cell) => {
                let function = self
                    .overlay
                    .expect("an overlay target came from the active overlay")
                    .call;
                match function {
                    OverlayCall::Runtime(function) => function(*cell, self, call, environment),
                    OverlayCall::Value(function) => {
                        function(*cell, self, call, environment).map(RuntimeValue::from)
                    }
                }
            }
        }?;
        Ok(unbox(value))
    }

    pub fn apply(
        &mut self,
        function: &RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        let callable = self.prepare_runtime_callable(function.clone(), &self.empty_environment());
        self.call_prepared(&callable, arguments)
    }

    /// Run synchronously with an additional foreign-function layer.
    /// Its owned closures may carry state local to this evaluation; it
    /// takes calls to its functions before the host's definitions, the
    /// overlay, and the layers it runs within.
    pub fn with_foreign_functions<T>(
        &mut self,
        functions: ForeignFunctions,
        run: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.foreign_scopes.push(functions);
        let result = run(self);
        self.foreign_scopes.pop();
        result
    }

    pub fn prepare_callable(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<PreparedCallable, Halt> {
        let callable = self.eval(expression, environment)?;
        Ok(self.prepare_runtime_callable(callable, environment))
    }

    /// Prepare an already evaluated callable without reifying its closure or
    /// evaluating the value a second time. It owns its retained code and captures.
    pub fn prepare_runtime_callable(
        &mut self,
        callable: RuntimeValue,
        environment: &Environment,
    ) -> PreparedCallable {
        let callable = match &callable.0 {
            RuntimeValueKind::Data(_)
            | RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_) => self
                .runtime_closure(&callable)
                .map(|closure| RuntimeValue::new(RuntimeValueKind::Closure(closure)))
                .unwrap_or(callable),
            _ => callable,
        };
        PreparedCallable {
            value: callable,
            environment: environment.clone(),
        }
    }

    pub fn call_prepared(
        &mut self,
        callable: &PreparedCallable,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        self.checked_call(|context| {
            context.burn()?;
            context.burn()?;
            let arguments: Vec<_> = arguments.into_iter().collect();
            match &callable.value.0 {
                RuntimeValueKind::Closure(closure) => {
                    context.call_closure(closure.clone(), &callable.value, &arguments)
                }
                RuntimeValueKind::Data(_)
                | RuntimeValueKind::Unboxed(_)
                | RuntimeValueKind::Record(_) => {
                    let value = &callable.value;
                    match callee_cell(value) {
                        Some(cell) => {
                            let call = runtime_call(value.clone(), arguments);
                            match context.foreign_target_cell(cell) {
                                Some(foreign) => {
                                    context.call_foreign(&foreign, &call, &callable.environment)
                                }
                                None => context.call_cell(cell, &call, &callable.environment, None),
                            }
                        }
                        None => Ok(not_callable(value)),
                    }
                }
                RuntimeValueKind::List(_) => Ok(not_callable(&callable.value)),
            }
        })
    }

    /// Apply runtime-held callable syntax. Grap parameters receive values;
    /// foreign functions receive argument expressions to interpret. Use
    /// `apply` when arguments must remain already-evaluated values for FFIs too.
    pub fn apply_expression(
        &mut self,
        function: &RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        // The call itself, as a call expression would charge it.
        self.burn()?;
        let arguments = arguments.into_iter().collect();
        let environment = self.empty_environment();
        if let Some(cell) = function.as_cell()
            && self.foreign_target_cell(cell).is_none()
        {
            self.apply_cell(cell, function.clone(), arguments, &environment)
        } else {
            self.checked_call(|context| {
                let callable = if function.as_cell().is_some() {
                    context.burn()?;
                    function.clone()
                } else {
                    let function = match &function.0 {
                        RuntimeValueKind::Data(value) => {
                            context.lower_source(value, OriginRoot::Input)
                        }
                        _ => context.lower_runtime_code(function),
                    };
                    context.eval(function, &environment)?
                };
                match context.try_apply_callable(callable.clone(), &arguments, &environment) {
                    Some(result) => result,
                    None => Ok(not_callable(&callable)),
                }
            })
        }
    }

    fn apply_cell(
        &mut self,
        cell: CellId,
        callable: RuntimeValue,
        arguments: Vec<(CellId, RuntimeValue)>,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        self.checked_call(|context| match context.host.resolve(cell) {
            Some(Definition::Foreign(definition)) => context.apply_foreign(
                &ResolvedForeign::Function(definition.implementation.clone()),
                callable,
                arguments,
                environment,
            ),
            Some(Definition::Value(value)) => {
                let expression = context.lower_source(&value, OriginRoot::Cell(cell));
                let callable = context.eval_definition(cell, expression, environment)?;
                if callable.absent_reason().is_some() {
                    Ok(callable)
                } else {
                    context
                        .try_apply_callable(callable.clone(), &arguments, environment)
                        .unwrap_or_else(|| Ok(not_callable(&callable)))
                }
            }
            None => Ok(missing_cell(cell)),
        })
    }

    /// Host calls bind argument values as a call expression binds its
    /// evaluated arguments: one step each, with a `function` parameter
    /// receiving the callee itself. A missing argument binds its absence
    /// without a step, naming no call: a host call has no source.
    fn call_closure(
        &mut self,
        closure: Closure,
        callable: &RuntimeValue,
        arguments: &[(CellId, RuntimeValue)],
    ) -> Result<RuntimeValue, Halt> {
        let bound = closure.params.iter().try_fold(
            Vec::with_capacity(closure.params.len()),
            |mut bound, &parameter| {
                let value = if parameter == vocabulary::FUNCTION {
                    Some(callable.clone())
                } else {
                    arguments
                        .iter()
                        .find(|(cell, _)| *cell == parameter)
                        .map(|(_, value)| unbox(value.clone()))
                };
                bound.push((
                    parameter,
                    match value {
                        Some(value) => {
                            self.burn()?;
                            value
                        }
                        None => absent::missing_argument(parameter, None).into(),
                    },
                ));
                Ok::<_, Halt>(bound)
            },
        )?;
        self.eval(closure.body, &closure.environment.extended_runtime(bound))
    }

    fn try_apply_callable(
        &mut self,
        callable: RuntimeValue,
        arguments: &[(CellId, RuntimeValue)],
        environment: &Environment,
    ) -> Option<Result<RuntimeValue, Halt>> {
        match self.runtime_closure(&callable) {
            Some(closure) => Some(self.call_closure(closure, &callable, arguments)),
            None => match callable.0 {
                RuntimeValueKind::Data(_)
                | RuntimeValueKind::Unboxed(_)
                | RuntimeValueKind::Record(_) => {
                    callee_cell(&callable).map(|cell| match self.foreign_target_cell(cell) {
                        Some(foreign) => self.apply_foreign(
                            &foreign,
                            callable,
                            arguments.iter().cloned(),
                            environment,
                        ),
                        None => self.apply_cell(cell, callable, arguments.to_vec(), environment),
                    })
                }
                RuntimeValueKind::List(_) | RuntimeValueKind::Closure(_) => None,
            },
        }
    }

    fn eval_grap_call(
        &mut self,
        closure: Closure,
        call: &Expression,
        calling_environment: &Environment,
        plan: &RefCell<Option<CallPlan>>,
    ) -> Result<RuntimeValue, Halt> {
        let cached = plan.borrow().as_ref().and_then(|cached| {
            (Rc::ptr_eq(&cached.params, &closure.params)
                && Rc::ptr_eq(&cached.table, &closure.environment.indices))
            .then(|| (cached.arguments.clone(), cached.indices.clone()))
        });
        let (arguments_plan, indices) = match cached {
            Some(cached) => cached,
            None => {
                let arguments: Rc<[arguments::Argument]> = closure
                    .params
                    .iter()
                    .map(|&parameter| call.argument(parameter))
                    .collect();
                let table = closure.environment.indices.clone();
                let indices: Rc<[CellIndex]> = closure
                    .params
                    .iter()
                    .map(|&parameter| binding_index(&table, parameter))
                    .collect();
                *plan.borrow_mut() = Some(CallPlan {
                    params: closure.params.clone(),
                    arguments: arguments.clone(),
                    table,
                    indices: indices.clone(),
                });
                (arguments, indices)
            }
        };
        // A missing argument binds its parameter to its absence, which
        // answers wherever the body uses it, so a body that doesn't still
        // has its value.
        let arguments = arguments_plan.iter().zip(indices.iter()).try_fold(
            Vec::with_capacity(closure.params.len()),
            |mut arguments, (argument, &index)| {
                arguments.push((
                    index,
                    match argument {
                        Ok(expression) => self.eval(expression.clone(), calling_environment)?,
                        Err(missing) => missing.clone(),
                    },
                ));
                Ok::<_, Halt>(arguments)
            },
        )?;
        self.eval(
            closure.body,
            &closure.environment.extended_indexed(arguments),
        )
    }

    pub(crate) fn runtime_closure(&mut self, value: &RuntimeValue) -> Option<Closure> {
        match &value.0 {
            RuntimeValueKind::Closure(closure) => Some(closure.clone()),
            RuntimeValueKind::Data(_)
            | RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_) => {
                (value.kind()? == vocabulary::kind::CLOSURE).then_some(())?;
                let lambda = value.field(vocabulary::LAMBDA)?;
                let params = lambda
                    .field(vocabulary::PARAMS)?
                    .list_values()?
                    .map(|value| value.as_cell())
                    .collect::<Option<Rc<[_]>>>()?;
                let origin = value
                    .field(source::vocabulary::BODY_ORIGIN)
                    .and_then(|value| source::read(value.as_value()))
                    .map(|origin| OriginId(Rc::new(OriginNode::Root(OriginRoot::Located(origin)))));
                let body = self.lower_runtime_code_at(&lambda.field(vocabulary::BODY)?, origin);
                let captured = value.field(vocabulary::ENVIRONMENT)?;
                let environment = match &captured.0 {
                    RuntimeValueKind::Data(value) => self.environment(value),
                    RuntimeValueKind::Record(fields) => Some(
                        self.empty_environment()
                            .extended_runtime(fields.iter().cloned()),
                    ),
                    _ => None,
                }?;
                Some(Closure {
                    params,
                    body,
                    environment,
                    fields: lambda.as_value().as_record()?.clone(),
                })
            }
            RuntimeValueKind::List(_) => None,
        }
    }
}

/// The cell calling `callable` calls: the cell it is, or the one a foreign
/// function is implemented under.
fn callee_cell(callable: &RuntimeValue) -> Option<CellId> {
    callable.as_cell().or_else(|| {
        (callable.kind()? == vocabulary::kind::FOREIGN_FUNCTION)
            .then(|| callable.field(vocabulary::IMPLEMENTATION)?.as_cell())?
    })
}

/// The absence a call answers when `callable` can't be called: an absent
/// `callable` as it is, since what failed before it is the reason.
pub(crate) fn not_callable(callable: &RuntimeValue) -> RuntimeValue {
    arguments::not(absent::NOT_CALLABLE, callable.clone())
}
