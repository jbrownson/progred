//! Editor conventions: well-known node ids the editor treats
//! specially. The data layer knows nothing of these — they are to gid
//! what syntax highlighting is to ASCII. Minted once via uuidgen
//! (CSPRNG) on 2026-07-05. The cons-list ids (head/tail/empty) were
//! retired 2026-07-06 for ordered-identity position labels.

use crate::sources::Sources;
use progred_graph::{
    Id, MutGid, NODE_SPACE, NUMBER_SPACE, NodeId, STRING_SPACE, position::POSITION_SPACE,
};
use std::rc::Rc;

pub const NAME: NodeId = NodeId::from_u128(0xf8ac_c21e_3635_4e5a_9702_1ee4_8d29_fed8);

/// The built-in library: names for the editor's well-known ids, read
/// under every document through [`Sources`] — never written, never
/// saved. What the NAME hardcode used to fake is data here, so
/// completion can offer it and the graph can label it like anything
/// else.
pub fn library() -> MutGid {
    let mut gid = MutGid::new();
    let name = Id::from(NAME);
    gid.set(NAME, name.clone(), Id::from("name"));
    for (space, text) in [
        (NODE_SPACE, "node"),
        (STRING_SPACE, "string"),
        (NUMBER_SPACE, "number"),
        (POSITION_SPACE, "position"),
    ] {
        gid.set(space, name.clone(), Id::from(text));
    }
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
    pub label: Option<Id>,
}

/// The editor's name policy: every display-name lookup goes through
/// this one function, making "what counts as a name" editor state —
/// expandable (other conventions, computed names) and disableable (a
/// strict raw view shows bare identities). The write side —
/// completion's name-your-new-node offer — stays on the `name`
/// convention until it needs to vary.
#[derive(Clone)]
pub struct Names(Rc<dyn Fn(&Sources, &Id) -> Option<Name>>);

impl Names {
    /// The `name` convention: a `name` edge to a string value, read
    /// through the document and its library alike — the well-known
    /// ids' own names are library data, not code.
    pub fn convention() -> Self {
        Self(Rc::new(|sources, id| {
            let label = Id::from(NAME);
            let text = sources.get(id, &label).and_then(Id::as_str)?;
            Some(Name {
                text: text.to_string(),
                label: Some(label),
            })
        }))
    }

    /// Names disabled: every identity reads as itself — the Raw view.
    pub fn none() -> Self {
        Self(Rc::new(|_, _| None))
    }

    pub fn of(&self, sources: &Sources, id: &Id) -> Option<Name> {
        (self.0)(sources, id)
    }
}

impl Default for Names {
    fn default() -> Self {
        Self::convention()
    }
}
