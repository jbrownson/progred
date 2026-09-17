//! Offline structural-sharing census. This never substitutes a program in the
//! renderer: it reconstructs expression identities from register instructions.
//! Copies/spills and register numbering disappear; operation/operand order,
//! variable identities and floating-point bits do not. HashMap confirms full
//! key equality, so fingerprints cannot silently merge different expressions.
use fidget_engine::{
    compiler::RegOp,
    context::{BinaryOpcode, UnaryOpcode},
    var::{Var, VarMap},
    vm::VmData,
};
use std::collections::{HashMap, HashSet};

use super::expression::{Id, Node};
const UNSET: Id = Id::MAX;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Statistics {
    pub programs: usize,
    pub register_ops: usize,
    pub outputs: usize,
    // Each category counts nodes, not recursively expanded subtree sizes.
    pub nodes_per_program: usize,
    pub distinct_expressions: usize,
    pub nodes_per_distinct_expression: usize,
    pub unique_nodes: usize,
    // Inputs, constants, unary operations, binary operations.
    pub unique_by_kind: [usize; 4],
}

#[derive(Default)]
pub(super) struct Subtrees {
    // These addresses only avoid recounting existing Arc sharing. Observed
    // functions must stay alive throughout the census (as the renderer does).
    programs: HashSet<usize>,
    nodes: Vec<Node>,
    interned: HashMap<Node, Id>,
    expressions: HashSet<Vec<Id>>,
    reachable: HashSet<Id>,
    statistics: Statistics,
}

impl Subtrees {
    fn intern(&mut self, node: Node) -> Id {
        let next = Id::try_from(self.nodes.len()).unwrap();
        assert_ne!(next, UNSET);
        *self.interned.entry(node).or_insert_with(|| {
            self.nodes.push(node);
            next
        })
    }

    fn constant(&mut self, value: f32) -> Id {
        self.intern(Node::Constant(value.to_bits()))
    }

    pub fn observe<const N: usize>(&mut self, data: &VmData<N>) {
        if !self.programs.insert(data as *const _ as usize) {
            return;
        }
        self.statistics.programs += 1;
        self.reconstruct(data.iter_asm(), &data.vars, data.output_count());
    }

    fn reconstruct(
        &mut self,
        ops: impl Iterator<Item = RegOp>,
        vars: &VarMap,
        output_count: usize,
    ) -> Vec<Id> {
        let mut variables = vec![None; vars.len()];
        for (var, index) in vars.iter() {
            variables[index] = Some(var);
        }
        let mut registers = [UNSET; 256];
        let mut spills = HashMap::new();
        let mut outputs = vec![UNSET; output_count];
        let read = |registers: &[Id; 256], register: u8| {
            let id = registers[register as usize];
            assert_ne!(id, UNSET, "read before register definition");
            id
        };
        for op in ops {
            self.statistics.register_ops += 1;
            use RegOp::*;
            // These macros only reduce the mechanical mapping of Fidget's
            // instruction set. They perform no algebraic simplification.
            macro_rules! unary {
                ($kind:ident, $out:expr, $arg:expr) => {{
                    let arg = read(&registers, $arg);
                    registers[$out as usize] = self.intern(Node::Unary(UnaryOpcode::$kind, arg));
                }};
            }
            macro_rules! binary {
                ($kind:ident, $out:expr, $a:expr, $b:expr) => {{
                    let a = $a;
                    let b = $b;
                    registers[$out as usize] = self.intern(Node::Binary(BinaryOpcode::$kind, a, b));
                }};
            }
            macro_rules! rr {
                ($kind:ident, $out:expr, $a:expr, $b:expr) => {
                    binary!($kind, $out, read(&registers, $a), read(&registers, $b))
                };
            }
            macro_rules! ri {
                ($kind:ident, $out:expr, $a:expr, $b:expr) => {
                    binary!($kind, $out, read(&registers, $a), self.constant($b))
                };
            }
            macro_rules! ir {
                ($kind:ident, $out:expr, $a:expr, $b:expr) => {
                    binary!($kind, $out, self.constant($b), read(&registers, $a))
                };
            }
            match op {
                Output(arg, output) => outputs[output as usize] = read(&registers, arg),
                Input(out, input) => {
                    registers[out as usize] =
                        self.intern(Node::Input(variables[input as usize].unwrap()));
                }
                CopyImm(out, value) => registers[out as usize] = self.constant(value),
                CopyReg(out, arg) => registers[out as usize] = read(&registers, arg),
                Store(arg, slot) => {
                    spills.insert(slot, read(&registers, arg));
                }
                Load(out, slot) => registers[out as usize] = spills[&slot],
                NegReg(o, a) => unary!(Neg, o, a),
                AbsReg(o, a) => unary!(Abs, o, a),
                RecipReg(o, a) => unary!(Recip, o, a),
                SqrtReg(o, a) => unary!(Sqrt, o, a),
                SquareReg(o, a) => unary!(Square, o, a),
                FloorReg(o, a) => unary!(Floor, o, a),
                CeilReg(o, a) => unary!(Ceil, o, a),
                RoundReg(o, a) => unary!(Round, o, a),
                SinReg(o, a) => unary!(Sin, o, a),
                CosReg(o, a) => unary!(Cos, o, a),
                TanReg(o, a) => unary!(Tan, o, a),
                AsinReg(o, a) => unary!(Asin, o, a),
                AcosReg(o, a) => unary!(Acos, o, a),
                AtanReg(o, a) => unary!(Atan, o, a),
                ExpReg(o, a) => unary!(Exp, o, a),
                LnReg(o, a) => unary!(Ln, o, a),
                NotReg(o, a) => unary!(Not, o, a),
                RandReg(o, a) => unary!(Rand, o, a),
                AddRegReg(o, a, b) => rr!(Add, o, a, b),
                MulRegReg(o, a, b) => rr!(Mul, o, a, b),
                DivRegReg(o, a, b) => rr!(Div, o, a, b),
                SubRegReg(o, a, b) => rr!(Sub, o, a, b),
                ModRegReg(o, a, b) => rr!(Mod, o, a, b),
                AtanRegReg(o, a, b) => rr!(Atan, o, a, b),
                CompareRegReg(o, a, b) => rr!(Compare, o, a, b),
                MixRegReg(o, a, b) => rr!(Mix, o, a, b),
                MinRegReg(o, a, b) => rr!(Min, o, a, b),
                MaxRegReg(o, a, b) => rr!(Max, o, a, b),
                AndRegReg(o, a, b) => rr!(And, o, a, b),
                OrRegReg(o, a, b) => rr!(Or, o, a, b),
                AddRegImm(o, a, b) => ri!(Add, o, a, b),
                MulRegImm(o, a, b) => ri!(Mul, o, a, b),
                DivRegImm(o, a, b) => ri!(Div, o, a, b),
                SubRegImm(o, a, b) => ri!(Sub, o, a, b),
                ModRegImm(o, a, b) => ri!(Mod, o, a, b),
                AtanRegImm(o, a, b) => ri!(Atan, o, a, b),
                CompareRegImm(o, a, b) => ri!(Compare, o, a, b),
                MixRegImm(o, a, b) => ri!(Mix, o, a, b),
                MinRegImm(o, a, b) => ri!(Min, o, a, b),
                MaxRegImm(o, a, b) => ri!(Max, o, a, b),
                AndRegImm(o, a, b) => ri!(And, o, a, b),
                OrRegImm(o, a, b) => ri!(Or, o, a, b),
                DivImmReg(o, a, b) => ir!(Div, o, a, b),
                SubImmReg(o, a, b) => ir!(Sub, o, a, b),
                ModImmReg(o, a, b) => ir!(Mod, o, a, b),
                AtanImmReg(o, a, b) => ir!(Atan, o, a, b),
                CompareImmReg(o, a, b) => ir!(Compare, o, a, b),
                MixImmReg(o, a, b) => ir!(Mix, o, a, b),
            }
        }
        assert!(outputs.iter().all(|id| *id != UNSET));
        // Count each reachable node once per program, not once per reference
        // or once per expanded subtree: a DAG can have many overlapping paths.
        let mut reachable = HashSet::new();
        let mut todo = outputs.clone();
        while let Some(id) = todo.pop() {
            if !reachable.insert(id) {
                continue;
            }
            match self.nodes[id as usize] {
                Node::Input(..) | Node::Constant(..) => {}
                Node::Unary(_, a) => todo.push(a),
                Node::Binary(_, a, b) => todo.extend([a, b]),
            }
        }
        self.statistics.outputs += outputs.len();
        self.statistics.nodes_per_program += reachable.len();
        if self.expressions.insert(outputs.clone()) {
            self.statistics.nodes_per_distinct_expression += reachable.len();
        }
        for id in reachable {
            if self.reachable.insert(id) {
                self.statistics.unique_by_kind[match self.nodes[id as usize] {
                    Node::Input(..) => 0,
                    Node::Constant(..) => 1,
                    Node::Unary(..) => 2,
                    Node::Binary(..) => 3,
                }] += 1;
            }
        }
        outputs
    }

    pub fn statistics(&self) -> Statistics {
        Statistics {
            distinct_expressions: self.expressions.len(),
            unique_nodes: self.reachable.len(),
            ..self.statistics
        }
    }

    pub fn report(&self, label: &str) {
        let stats = self.statistics();
        eprintln!("subtrees {label}: {stats:?}");
        eprintln!(
            "subtree storage: register_payload={} B; node_size={} B; independent_graph_payload={} B; distinct_expression_graph_payload={} B; shared_node_payload={} B; all_output_handles={} B; node_vec_capacity={} B; interning_capacity={} entries",
            stats.register_ops * size_of::<RegOp>(),
            size_of::<Node>(),
            stats.nodes_per_program * size_of::<Node>(),
            stats.nodes_per_distinct_expression * size_of::<Node>(),
            stats.unique_nodes * size_of::<Node>(),
            stats.outputs * size_of::<Id>(),
            self.nodes.capacity() * size_of::<Node>(),
            self.interned.capacity(),
        );
    }
}

#[test]
fn subtree_census_shares_branches_between_different_programs() {
    let mut vars = VarMap::new();
    vars.insert(Var::X);
    let mut census = Subtrees::default();
    let first = census.reconstruct(
        [
            RegOp::Input(0, 0),
            RegOp::SquareReg(0, 0),
            RegOp::AddRegImm(1, 0, 1.0),
            RegOp::Output(1, 0),
        ]
        .into_iter(),
        &vars,
        1,
    );
    let second = census.reconstruct(
        [
            RegOp::Input(3, 0),
            RegOp::SquareReg(7, 3),
            RegOp::Store(7, 512),
            RegOp::CopyImm(7, 123.0),
            RegOp::Load(2, 512),
            RegOp::CopyReg(4, 2),
            RegOp::AddRegImm(4, 4, 2.0),
            RegOp::Output(4, 0),
        ]
        .into_iter(),
        &vars,
        1,
    );
    assert_ne!(first, second);
    let Node::Binary(_, a, _) = census.nodes[first[0] as usize] else {
        panic!()
    };
    let Node::Binary(_, b, _) = census.nodes[second[0] as usize] else {
        panic!()
    };
    assert_eq!(
        a, b,
        "shared square survives register renaming, copies, and spills"
    );
    assert_eq!(census.statistics().nodes_per_program, 8);
    assert_eq!(census.statistics().distinct_expressions, 2);
    assert_eq!(
        census.statistics().unique_nodes,
        6,
        "unused register writes aren't retained subtrees"
    );
}

#[test]
fn subtree_census_preserves_constants_variables_and_operand_order() {
    let mut census = Subtrees::default();
    for (a, b) in [
        (0.0, -0.0),
        (f32::INFINITY, f32::NEG_INFINITY),
        (f32::from_bits(0x7fc00001), f32::from_bits(0x7fc00002)),
    ] {
        assert_ne!(census.constant(a), census.constant(b));
    }
    let mut vars = VarMap::new();
    vars.insert(Var::X);
    vars.insert(Var::Y);
    let mut reversed = VarMap::new();
    reversed.insert(Var::Y);
    reversed.insert(Var::X);
    let mut root = |input, vars: &VarMap, op| {
        census.reconstruct(
            [RegOp::Input(0, input), op, RegOp::Output(0, 0)].into_iter(),
            vars,
            1,
        )
    };
    let a = root(0, &vars, RegOp::SubRegImm(0, 0, 1.0));
    assert_eq!(a, root(1, &reversed, RegOp::SubRegImm(0, 0, 1.0)));
    assert_ne!(a, root(0, &reversed, RegOp::SubRegImm(0, 0, 1.0)));
    assert_ne!(a, root(0, &vars, RegOp::SubImmReg(0, 0, 1.0)));
}

#[test]
fn subtree_census_matches_compiled_math_and_existing_sharing() {
    use fidget_engine::{Context, context::Tree};
    let x = Tree::x();
    let y = Tree::y();
    let z = Tree::z();
    let tree = (x.clone().square() + y.clone().sin())
        .max(y.clone().square() + z.clone().cos())
        .min(z.square() + x.sin());
    let mut ctx = Context::new();
    let node = ctx.import(&tree);
    let compact = VmData::<3>::new(&ctx, &[node]).unwrap();
    let roomy = VmData::<255>::new(&ctx, &[node]).unwrap();
    assert!(compact.iter_asm().any(|op| matches!(op, RegOp::Store(..))));

    let mut census = Subtrees::default();
    census.observe(&compact);
    census.observe(&roomy);
    census.observe(&compact); // Already shared storage isn't a new copy.
    assert_eq!(census.statistics().programs, 2);
    assert_eq!(census.statistics().distinct_expressions, 1);
    let root = census.expressions.iter().next().unwrap()[0];
    for [x, y, z] in [[0.0, 0.0, 0.0], [1.25, -0.75, 0.5], [-2.0, 0.3, 4.0]] {
        let mut values = Vec::new();
        for node in &census.nodes {
            values.push(match *node {
                Node::Input(Var::X) => x,
                Node::Input(Var::Y) => y,
                Node::Input(Var::Z) => z,
                Node::Input(Var::V(..)) => unreachable!(),
                Node::Constant(bits) => f32::from_bits(bits),
                Node::Unary(op, a) => op.eval(values[a as usize]),
                Node::Binary(op, a, b) => op.eval(values[a as usize], values[b as usize]),
            });
        }
        assert_eq!(
            values[root as usize].to_bits(),
            ctx.eval_xyz(node, x, y, z).unwrap().to_bits()
        );
    }
}
