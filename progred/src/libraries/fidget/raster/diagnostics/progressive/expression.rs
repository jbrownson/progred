use fidget_engine::{
    context::{BinaryOpcode, UnaryOpcode},
    var::Var,
};

pub(super) type Id = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Node {
    Input(Var),
    Constant(u32),
    Unary(UnaryOpcode, Id),
    Binary(BinaryOpcode, Id, Id),
}

impl Node {
    pub fn children(self) -> impl DoubleEndedIterator<Item = Id> {
        match self {
            Self::Input(..) | Self::Constant(..) => [None, None],
            Self::Unary(_, a) => [Some(a), None],
            Self::Binary(_, a, b) => [Some(a), Some(b)],
        }
        .into_iter()
        .flatten()
    }
    pub fn choice(self) -> bool {
        matches!(
            self,
            Self::Binary(
                BinaryOpcode::Min | BinaryOpcode::Max | BinaryOpcode::And | BinaryOpcode::Or,
                ..
            )
        )
    }
}
