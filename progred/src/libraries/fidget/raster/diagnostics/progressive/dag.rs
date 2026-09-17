//! Test-only persistent expressions. Scene roots form one shared immutable
//! arena; each image tile owns its specialization arena. This deliberately
//! forgoes cross-tile interning to avoid serializing workers behind a global
//! lock. Regions retain only root IDs, never compiled instruction tapes.
use super::expression::{Id, Node};
use fidget_engine::{
    compiler::{SsaOp, SsaTape},
    context::{BinaryOpcode, UnaryOpcode},
    var::VarMap,
    vm::{Choice, VmTrace},
};
use std::{collections::HashMap, sync::Arc};

// Reused by one worker, including across tile arenas. Stamps distinguish each
// traversal, so neither stale IDs nor values from another tile can leak in.
#[derive(Default)]
pub(super) struct Workspace {
    epoch: u32,
    seen: Vec<u32>,
    values: Vec<Id>,
    selected: Vec<Id>,
    todo: Vec<(Id, bool)>,
    order: Vec<Id>,
}

impl Workspace {
    fn begin(&mut self, size: usize) {
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 {
            self.seen.fill(0);
            self.epoch = 1;
        }
        if self.seen.len() < size {
            self.seen.resize(size, 0);
            self.values.resize(size, 0);
            self.selected.resize(size, Id::MAX);
        }
        self.todo.clear();
        self.order.clear();
    }
}

#[derive(Default)]
pub(super) struct Graph {
    base: Option<Arc<Graph>>,
    nodes: Vec<Node>,
    index: HashMap<Node, Id>,
    offset: usize,
}

// Temporary correspondence between one compiled tape and its graph. Its order
// matches the tape's forward evaluation (including choice/trace order).
#[derive(Clone)]
pub(super) struct Compiled {
    pub root: Id,
    choices: Vec<Id>,
}

impl Graph {
    pub fn fork(base: &Arc<Self>) -> Self {
        assert!(base.base.is_none());
        Self {
            base: Some(base.clone()),
            offset: base.nodes.len(),
            ..Self::default()
        }
    }
    fn get(&self, id: Id) -> Node {
        if (id as usize) < self.offset {
            self.base.as_ref().unwrap().get(id)
        } else {
            self.nodes[id as usize - self.offset]
        }
    }
    fn intern(&mut self, node: Node) -> Id {
        if let Some(id) = self.base.as_ref().and_then(|base| base.index.get(&node)) {
            return *id;
        }
        let id = Id::try_from(self.offset + self.nodes.len()).unwrap();
        *self.index.entry(node).or_insert_with(|| {
            self.nodes.push(node);
            id
        })
    }
    pub fn sizes(&self) -> (usize, usize) {
        (self.nodes.len(), self.nodes.capacity() * size_of::<Node>())
    }

    pub fn import(&mut self, tape: &SsaTape, vars: &VarMap) -> Id {
        assert_eq!(tape.output_count, 1);
        let mut inputs = vec![None; vars.len()];
        for (var, slot) in vars.iter() {
            inputs[slot] = Some(var);
        }
        let mut slots = vec![Id::MAX; tape.tape.len()];
        let mut root = None;
        for op in tape.tape.iter().rev().copied() {
            use SsaOp::*;
            let read = |slot: u32| {
                let id = slots[slot as usize];
                assert_ne!(id, Id::MAX);
                id
            };
            macro_rules! unary {
                ($op:ident, $a:expr) => {
                    Node::Unary(UnaryOpcode::$op, read($a))
                };
            }
            macro_rules! rr {
                ($op:ident, $a:expr, $b:expr) => {
                    Node::Binary(BinaryOpcode::$op, read($a), read($b))
                };
            }
            macro_rules! ri {
                ($op:ident, $a:expr, $b:expr) => {
                    Node::Binary(
                        BinaryOpcode::$op,
                        read($a),
                        self.intern(Node::Constant($b.to_bits())),
                    )
                };
            }
            macro_rules! ir {
                ($op:ident, $a:expr, $b:expr) => {
                    Node::Binary(
                        BinaryOpcode::$op,
                        self.intern(Node::Constant($b.to_bits())),
                        read($a),
                    )
                };
            }
            let node = match op {
                Output(arg, output) => {
                    assert_eq!(output, 0);
                    root = Some(read(arg));
                    continue;
                }
                CopyReg(out, arg) => {
                    slots[out as usize] = read(arg);
                    continue;
                }
                Input(_, slot) => Node::Input(inputs[slot as usize].unwrap()),
                CopyImm(_, value) => Node::Constant(value.to_bits()),
                NegReg(_, a) => unary!(Neg, a),
                AbsReg(_, a) => unary!(Abs, a),
                RecipReg(_, a) => unary!(Recip, a),
                SqrtReg(_, a) => unary!(Sqrt, a),
                SquareReg(_, a) => unary!(Square, a),
                FloorReg(_, a) => unary!(Floor, a),
                CeilReg(_, a) => unary!(Ceil, a),
                RoundReg(_, a) => unary!(Round, a),
                SinReg(_, a) => unary!(Sin, a),
                CosReg(_, a) => unary!(Cos, a),
                TanReg(_, a) => unary!(Tan, a),
                AsinReg(_, a) => unary!(Asin, a),
                AcosReg(_, a) => unary!(Acos, a),
                AtanReg(_, a) => unary!(Atan, a),
                ExpReg(_, a) => unary!(Exp, a),
                LnReg(_, a) => unary!(Ln, a),
                NotReg(_, a) => unary!(Not, a),
                RandReg(_, a) => unary!(Rand, a),
                AddRegReg(_, a, b) => rr!(Add, a, b),
                MulRegReg(_, a, b) => rr!(Mul, a, b),
                DivRegReg(_, a, b) => rr!(Div, a, b),
                SubRegReg(_, a, b) => rr!(Sub, a, b),
                ModRegReg(_, a, b) => rr!(Mod, a, b),
                AtanRegReg(_, a, b) => rr!(Atan, a, b),
                CompareRegReg(_, a, b) => rr!(Compare, a, b),
                MixRegReg(_, a, b) => rr!(Mix, a, b),
                MinRegReg(_, a, b) => rr!(Min, a, b),
                MaxRegReg(_, a, b) => rr!(Max, a, b),
                AndRegReg(_, a, b) => rr!(And, a, b),
                OrRegReg(_, a, b) => rr!(Or, a, b),
                AddRegImm(_, a, b) => ri!(Add, a, b),
                MulRegImm(_, a, b) => ri!(Mul, a, b),
                DivRegImm(_, a, b) => ri!(Div, a, b),
                SubRegImm(_, a, b) => ri!(Sub, a, b),
                ModRegImm(_, a, b) => ri!(Mod, a, b),
                AtanRegImm(_, a, b) => ri!(Atan, a, b),
                CompareRegImm(_, a, b) => ri!(Compare, a, b),
                MixRegImm(_, a, b) => ri!(Mix, a, b),
                MinRegImm(_, a, b) => ri!(Min, a, b),
                MaxRegImm(_, a, b) => ri!(Max, a, b),
                AndRegImm(_, a, b) => ri!(And, a, b),
                OrRegImm(_, a, b) => ri!(Or, a, b),
                DivImmReg(_, a, b) => ir!(Div, a, b),
                SubImmReg(_, a, b) => ir!(Sub, a, b),
                ModImmReg(_, a, b) => ir!(Mod, a, b),
                AtanImmReg(_, a, b) => ir!(Atan, a, b),
                CompareImmReg(_, a, b) => ir!(Compare, a, b),
                MixImmReg(_, a, b) => ir!(Mix, a, b),
            };
            slots[op.output().unwrap() as usize] = self.intern(node);
        }
        root.unwrap()
    }

    pub fn lower(&self, root: Id, scratch: &mut Workspace) -> (SsaTape, Arc<VarMap>, Compiled) {
        scratch.begin(self.offset + self.nodes.len());
        scratch.todo.push((root, false));
        while let Some((id, ready)) = scratch.todo.pop() {
            if scratch.seen[id as usize] == scratch.epoch {
                continue;
            }
            if !ready {
                scratch.todo.push((id, true));
                let node = self.get(id);
                if let Node::Binary(_, a, b) = node
                    && matches!(self.get(b), Node::Constant(_))
                {
                    // Fidget has a right-immediate form for every binary op.
                    // Keep operand order; no swapping or constant folding.
                    scratch.todo.push((a, false));
                } else {
                    scratch
                        .todo
                        .extend(node.children().rev().map(|child| (child, false)));
                }
                continue;
            }
            // Emit dependencies next to their consumers rather than sorting
            // by arena allocation order, which needlessly lengthens lifetimes.
            scratch.seen[id as usize] = scratch.epoch;
            scratch.values[id as usize] = scratch.order.len() as u32;
            scratch.order.push(id);
        }
        let registers = &scratch.values;
        let mut tape = Vec::with_capacity(scratch.order.len() + 1);
        let mut vars = VarMap::new();
        let mut choice_count = 0;
        for id in &scratch.order {
            let out = registers[*id as usize];
            use SsaOp::*;
            let node = self.get(*id);
            choice_count += usize::from(node.choice());
            let op = match node {
                Node::Input(var) => {
                    vars.insert(var);
                    Input(out, vars[&var] as u32)
                }
                Node::Constant(bits) => CopyImm(out, f32::from_bits(bits)),
                Node::Unary(op, a) => {
                    let a = registers[a as usize];
                    use UnaryOpcode::*;
                    match op {
                        Neg => NegReg(out, a),
                        Abs => AbsReg(out, a),
                        Recip => RecipReg(out, a),
                        Sqrt => SqrtReg(out, a),
                        Square => SquareReg(out, a),
                        Floor => FloorReg(out, a),
                        Ceil => CeilReg(out, a),
                        Round => RoundReg(out, a),
                        Sin => SinReg(out, a),
                        Cos => CosReg(out, a),
                        Tan => TanReg(out, a),
                        Asin => AsinReg(out, a),
                        Acos => AcosReg(out, a),
                        Atan => AtanReg(out, a),
                        Exp => ExpReg(out, a),
                        Ln => LnReg(out, a),
                        Not => NotReg(out, a),
                        Rand => RandReg(out, a),
                    }
                }
                Node::Binary(op, a, b) => {
                    let a = registers[a as usize];
                    macro_rules! binary {
                        ($reg:ident, $imm:ident) => {
                            match self.get(b) {
                                Node::Constant(bits) => $imm(out, a, f32::from_bits(bits)),
                                _ => $reg(out, a, registers[b as usize]),
                            }
                        };
                    }
                    use BinaryOpcode::*;
                    match op {
                        Add => binary!(AddRegReg, AddRegImm),
                        Sub => binary!(SubRegReg, SubRegImm),
                        Mul => binary!(MulRegReg, MulRegImm),
                        Div => binary!(DivRegReg, DivRegImm),
                        Atan => binary!(AtanRegReg, AtanRegImm),
                        Min => binary!(MinRegReg, MinRegImm),
                        Max => binary!(MaxRegReg, MaxRegImm),
                        Compare => binary!(CompareRegReg, CompareRegImm),
                        Mod => binary!(ModRegReg, ModRegImm),
                        And => binary!(AndRegReg, AndRegImm),
                        Or => binary!(OrRegReg, OrRegImm),
                        Mix => binary!(MixRegReg, MixRegImm),
                    }
                }
            };
            tape.push(op);
        }
        tape.push(SsaOp::Output(registers[root as usize], 0));
        tape.reverse();
        (
            SsaTape {
                tape,
                choice_count,
                output_count: 1,
            },
            Arc::new(vars),
            Compiled {
                root,
                choices: scratch
                    .order
                    .iter()
                    .copied()
                    .filter(|id| self.get(*id).choice())
                    .collect(),
            },
        )
    }

    pub fn specialize(
        &mut self,
        compiled: &Compiled,
        trace: &VmTrace,
        scratch: &mut Workspace,
    ) -> Id {
        assert_eq!(compiled.choices.len(), trace.as_slice().len());
        if trace.as_slice().iter().all(|c| *c == Choice::Both) {
            return compiled.root;
        }
        scratch.begin(self.offset + self.nodes.len());
        for (id, choice) in compiled.choices.iter().zip(trace.as_slice()) {
            let Node::Binary(_, a, b) = self.get(*id) else {
                unreachable!()
            };
            scratch.selected[*id as usize] = match choice {
                Choice::Left => a,
                Choice::Right => b,
                Choice::Both => Id::MAX,
                Choice::Unknown => panic!("unknown interval choice"),
            };
        }
        scratch.todo.push((compiled.root, false));
        while let Some((id, ready)) = scratch.todo.pop() {
            if scratch.seen[id as usize] == scratch.epoch {
                continue;
            }
            let node = self.get(id);
            let selected = node
                .choice()
                .then(|| scratch.selected[id as usize])
                .filter(|id| *id != Id::MAX);
            if !ready {
                scratch.todo.push((id, true));
                if let Some(child) = selected {
                    scratch.todo.push((child, false));
                } else {
                    scratch
                        .todo
                        .extend(node.children().map(|child| (child, false)));
                }
                continue;
            }
            let replacement = if let Some(child) = selected {
                scratch.values[child as usize]
            } else {
                let new = match node {
                    Node::Input(..) | Node::Constant(..) => node,
                    Node::Unary(op, a) => Node::Unary(op, scratch.values[a as usize]),
                    Node::Binary(op, a, b) => {
                        Node::Binary(op, scratch.values[a as usize], scratch.values[b as usize])
                    }
                };
                if new == node { id } else { self.intern(new) }
            };
            scratch.values[id as usize] = replacement;
            scratch.seen[id as usize] = scratch.epoch;
        }
        scratch.values[compiled.root as usize]
    }
}

#[test]
fn persistent_dag_reuses_unchanged_nodes_and_keeps_old_roots() {
    use fidget_engine::var::Var;
    let mut base = Graph::default();
    let x = base.intern(Node::Input(Var::X));
    let y = base.intern(Node::Input(Var::Y));
    let xx = base.intern(Node::Unary(UnaryOpcode::Square, x));
    let yy = base.intern(Node::Unary(UnaryOpcode::Square, y));
    let root = base.intern(Node::Binary(BinaryOpcode::Min, xx, yy));
    let mut graph = Graph::fork(&Arc::new(base));
    let mut scratch = Workspace::default();
    let (ssa, _, compiled) = graph.lower(root, &mut scratch);
    let mut trace = VmTrace::default();
    trace.resize(ssa.choice_count, Choice::Both);
    assert_eq!(graph.specialize(&compiled, &trace, &mut scratch), root);
    trace.fill(Choice::Left);
    assert_eq!(graph.specialize(&compiled, &trace, &mut scratch), xx);
    trace.fill(Choice::Right);
    assert_eq!(graph.specialize(&compiled, &trace, &mut scratch), yy);
    assert_eq!(graph.get(root), Node::Binary(BinaryOpcode::Min, xx, yy));
    assert_eq!(
        graph.sizes().0,
        0,
        "these versions only need existing nodes"
    );
}

#[test]
fn lowering_keeps_independent_branches_local_and_preserves_expression_bits() {
    use fidget_engine::{compiler::RegOp, var::Var, vm::VmData};
    let mut graph = Graph::default();
    let x = graph.intern(Node::Input(Var::X));
    // Allocate every arithmetic branch before allocating their union. Arena
    // order would leave all these partial results live simultaneously.
    let branches: Vec<_> = (0..40)
        .map(|i| {
            let c = graph.intern(Node::Constant((i as f32).to_bits()));
            let shifted = graph.intern(Node::Binary(BinaryOpcode::Sub, x, c));
            graph.intern(Node::Unary(UnaryOpcode::Square, shifted))
        })
        .collect();
    let root = branches
        .into_iter()
        .reduce(|a, b| graph.intern(Node::Binary(BinaryOpcode::Min, a, b)))
        .unwrap();
    let mut scratch = Workspace::default();
    let (ssa, vars, _) = graph.lower(root, &mut scratch);
    let function = VmData::<8>::from_ssa(ssa, vars);
    assert!(
        !function
            .iter_asm()
            .any(|op| matches!(op, RegOp::Load(..) | RegOp::Store(..)))
    );
    assert_eq!(graph.import(function.ssa(), &function.vars), root);

    // All right-immediate operations preserve their exact expression. Include
    // signed zero and distinct NaNs without relying on floating-point equality.
    for bits in [
        0,
        (-0.0f32).to_bits(),
        f32::INFINITY.to_bits(),
        0x7fc00001,
        0x7fc00002,
    ] {
        let constant = graph.intern(Node::Constant(bits));
        for op in [
            BinaryOpcode::Add,
            BinaryOpcode::Sub,
            BinaryOpcode::Mul,
            BinaryOpcode::Div,
            BinaryOpcode::Atan,
            BinaryOpcode::Min,
            BinaryOpcode::Max,
            BinaryOpcode::Compare,
            BinaryOpcode::Mod,
            BinaryOpcode::And,
            BinaryOpcode::Or,
            BinaryOpcode::Mix,
        ] {
            for (a, b) in [(x, constant), (constant, x), (constant, constant)] {
                let root = graph.intern(Node::Binary(op, a, b));
                let (ssa, vars, _) = graph.lower(root, &mut scratch);
                assert_eq!(graph.import(&ssa, &vars), root);
            }
        }
        let (ssa, vars, _) = graph.lower(constant, &mut scratch);
        assert_eq!(graph.import(&ssa, &vars), constant);
    }
}

#[test]
fn specialization_skips_discarded_branches_and_scratch_survives_arena_changes() {
    use fidget_engine::var::Var;
    let mut base = Graph::default();
    let x = base.intern(Node::Input(Var::X));
    let y = base.intern(Node::Input(Var::Y));
    let choice = base.intern(Node::Binary(BinaryOpcode::Min, x, y));
    let discarded = base.intern(Node::Unary(UnaryOpcode::Square, choice));
    let root = base.intern(Node::Binary(BinaryOpcode::Max, x, discarded));
    let base = Arc::new(base);
    let mut first = Graph::fork(&base);
    let mut scratch = Workspace::default();
    let (_, _, compiled) = first.lower(root, &mut scratch);
    let mut trace = VmTrace::default();
    trace.resize(compiled.choices.len(), Choice::Left);
    assert_eq!(first.specialize(&compiled, &trace, &mut scratch), x);
    assert_eq!(first.sizes().0, 0, "do not rebuild the discarded square");

    // The same numeric ID denotes different immutable nodes in these two
    // tile arenas. Neither scratch entries nor a wrapped stamp may alias them.
    let mut second = Graph::fork(&base);
    let first_root = first.intern(Node::Unary(UnaryOpcode::Neg, x));
    let second_root = second.intern(Node::Unary(UnaryOpcode::Abs, y));
    assert_eq!(first_root, second_root);
    let (before, before_vars, _) = first.lower(first_root, &mut scratch);
    let (other, other_vars, _) = second.lower(second_root, &mut scratch);
    let mut check = Graph::default();
    let before = check.import(&before, &before_vars);
    let other = check.import(&other, &other_vars);
    assert_ne!(before, other);
    scratch.epoch = u32::MAX;
    let (after, after_vars, _) = first.lower(first_root, &mut scratch);
    assert_eq!(before, check.import(&after, &after_vars));
}
