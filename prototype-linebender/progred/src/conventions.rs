//! Editor conventions: well-known node ids the editor treats
//! specially. The data layer knows nothing of these — they are to gid
//! what syntax highlighting is to ASCII. Minted once via uuidgen
//! (CSPRNG) on 2026-07-05. The cons-list ids (head/tail/empty) were
//! retired 2026-07-06 for ordered-identity position labels.

use progred_graph::{Gid, Id, MutGid, NodeId};
use std::rc::Rc;

pub const NAME: NodeId = NodeId::from_u128(0xf8ac_c21e_3635_4e5a_9702_1ee4_8d29_fed8);

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
pub struct Names(Rc<dyn Fn(&MutGid, &Id) -> Option<Name>>);

impl Names {
    /// The `name` convention: a `name` edge to a string value. The
    /// convention also knows its own node — `NAME` reads as "name"
    /// with no edge behind it, so a fresh document needs no
    /// self-description (a stored name still wins). The eventual home
    /// for this kind of fact is a library gid layered under the
    /// document.
    pub fn convention() -> Self {
        Self(Rc::new(|gid, id| {
            let label = Id::from(NAME);
            if let Some(text) = gid.get(id, &label).and_then(Id::as_str) {
                return Some(Name {
                    text: text.to_string(),
                    label: Some(label),
                });
            }
            (id == &label).then(|| Name {
                text: "name".to_string(),
                label: None,
            })
        }))
    }

    /// Names disabled: every identity reads as itself — the Raw view.
    pub fn none() -> Self {
        Self(Rc::new(|_, _| None))
    }

    pub fn of(&self, gid: &MutGid, id: &Id) -> Option<Name> {
        (self.0)(gid, id)
    }
}

impl Default for Names {
    fn default() -> Self {
        Self::convention()
    }
}
