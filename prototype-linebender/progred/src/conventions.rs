//! Editor conventions: well-known node ids the editor treats
//! specially. The data layer knows nothing of these — they are to the
//! graph what syntax highlighting is to ASCII. NAME minted via
//! uuidgen (CSPRNG) 2026-07-05.

use crate::sources::Sources;
use progred_graph::{Atom, MutGid, NodeId, Value};
use std::rc::Rc;

pub const NAME: NodeId = NodeId::from_u128(0xf8ac_c21e_3635_4e5a_9702_1ee4_8d29_fed8);

/// The built-in library: names for the editor's well-known ids, read
/// under every document through [`Sources`] — never written, never
/// saved — so completion offers the conventions and the graph labels
/// them like anything else.
pub fn library() -> MutGid {
    let mut gid = MutGid::new();
    gid.set(NAME, Atom::Node(NAME), Value::from("name"));
    gid
}

/// A node's display name, and where it came from: a name is not just
/// text — a name-aware projection must know which edge the name
/// CONSUMED, so it can skip that edge in its ordinary listing and
/// project it itself (the tree's header), keeping it selectable and
/// editable there.
pub struct Name {
    pub text: String,
    /// The edge the name reads from; `None` for a name not stored on
    /// a single edge (computed names consume nothing).
    pub label: Option<Atom>,
}

/// The editor's name policy: every display-name lookup goes through
/// this one function, making "what counts as a name" editor state —
/// expandable (other conventions, computed names). The Raw view is
/// not a policy of its own: lookups derive from the editor's one raw
/// bit and skip the policy entirely, so nothing is ever swapped. The
/// write side — completion's name-your-new-node offer — stays on the
/// `name` convention until it needs to vary.
#[derive(Clone)]
pub struct Names(Rc<dyn Fn(&Sources, NodeId) -> Option<Name>>);

impl Names {
    /// The `name` convention: a `name` edge to a string value, read
    /// through the document and its library alike — the well-known
    /// ids' own names are library data, not code.
    pub fn convention() -> Self {
        Self(Rc::new(|sources, node| {
            let label = Atom::Node(NAME);
            let text = sources.get(node, &label)?.as_str()?;
            Some(Name {
                text: text.to_string(),
                label: Some(label),
            })
        }))
    }

    pub fn of(&self, sources: &Sources, node: NodeId) -> Option<Name> {
        (self.0)(sources, node)
    }
}

impl Default for Names {
    fn default() -> Self {
        Self::convention()
    }
}
