//! Environments: the bindings a closure or call sees, keyed by interned cell
//! indices.

use crate::eval::unbox;
use crate::reify;
use crate::runtime::RuntimeValue;
use gid::{CellId, Value};
use std::cell::RefCell;
use std::rc::Rc;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CellIndex(pub(crate) usize);

/// Cell ids are minted from an OS CSPRNG, so folding their bytes is
/// already a uniform hash; SipHash would only add per-lookup cost.
#[derive(Debug, Default)]
struct FoldHasher(u64);

impl std::hash::Hasher for FoldHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0_u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.0 ^= u64::from_le_bytes(word);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Default)]
pub(crate) struct CellIndexTable {
    by_cell:
        std::collections::HashMap<CellId, CellIndex, std::hash::BuildHasherDefault<FoldHasher>>,
    cells: Vec<CellId>,
    /// Whether an environment over this table has bound each index. A
    /// reference to a cell nothing binds, a global's, misses at once
    /// instead of searching every frame.
    bound: Vec<bool>,
}

impl CellIndexTable {
    fn intern(&mut self, cell: CellId) -> CellIndex {
        if let Some(index) = self.by_cell.get(&cell) {
            *index
        } else {
            let index = CellIndex(self.cells.len());
            self.cells.push(cell);
            self.bound.push(false);
            self.by_cell.insert(cell, index);
            index
        }
    }

    pub(crate) fn cell(&self, index: CellIndex) -> CellId {
        self.cells[index.0]
    }

    fn index(&self, cell: CellId) -> Option<CellIndex> {
        self.by_cell.get(&cell).copied()
    }
}

pub(crate) type CellIndices = Rc<RefCell<CellIndexTable>>;

pub(crate) fn cell_index(indices: &CellIndices, cell: CellId) -> CellIndex {
    indices.borrow_mut().intern(cell)
}

/// `cell`'s index in `indices`, marked bound, for a binding.
pub(crate) fn binding_index(indices: &CellIndices, cell: CellId) -> CellIndex {
    let mut table = indices.borrow_mut();
    let index = table.intern(cell);
    table.bound[index.0] = true;
    index
}

#[derive(Debug, Clone, Default)]
pub struct Environment {
    pub(crate) indices: CellIndices,
    pub(crate) frame: Option<Rc<EnvironmentFrame>>,
}

#[derive(Debug, Clone)]
pub(crate) struct EnvironmentFrame {
    parent: Option<Rc<EnvironmentFrame>>,
    pub(crate) bindings: Vec<(CellIndex, RuntimeValue)>,
}

/// An environment's frames, innermost first.
pub(crate) fn frames(environment: &Environment) -> impl Iterator<Item = &EnvironmentFrame> {
    std::iter::successors(environment.frame.as_deref(), |frame| {
        frame.parent.as_deref()
    })
}

impl Environment {
    pub(crate) fn with_indices(indices: CellIndices) -> Self {
        Self {
            indices,
            frame: None,
        }
    }

    pub(crate) fn get_runtime(&self, cell: CellId) -> Option<&RuntimeValue> {
        let index = self.indices.borrow().index(cell)?;
        self.get_index(index)
    }

    /// `index` is `cell`'s slot in `indices`, resolved when the reference
    /// was compiled; an environment built over another table looks it up.
    pub(crate) fn get_at(
        &self,
        indices: &CellIndices,
        cell: CellId,
        index: CellIndex,
    ) -> Option<&RuntimeValue> {
        if Rc::ptr_eq(&self.indices, indices) {
            self.get_index(index)
        } else {
            self.get_runtime(cell)
        }
    }

    fn get_index(&self, index: CellIndex) -> Option<&RuntimeValue> {
        if self.indices.borrow().bound[index.0] {
            frames(self).find_map(|frame| {
                frame
                    .bindings
                    .iter()
                    .rev()
                    .find_map(|(binding, value)| (*binding == index).then_some(value))
            })
        } else {
            None
        }
    }

    pub(crate) fn extended(&self, bindings: impl IntoIterator<Item = (CellId, Value)>) -> Self {
        self.extended_runtime(
            bindings
                .into_iter()
                .map(|(cell, value)| (cell, RuntimeValue::from(value))),
        )
    }

    pub fn extended_runtime(
        &self,
        bindings: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Self {
        self.extended_indexed(
            bindings
                .into_iter()
                .map(|(cell, value)| (binding_index(&self.indices, cell), value)),
        )
    }

    /// Add bindings at the innermost level in place, copying the frame
    /// only while another environment still shares it. Lookup and
    /// reification are the same as through [`Self::extended_runtime`];
    /// sequential binders avoid a frame allocation per binding.
    pub fn push_runtime(&mut self, bindings: impl IntoIterator<Item = (CellId, RuntimeValue)>) {
        let indices = self.indices.clone();
        self.push_indexed(
            bindings
                .into_iter()
                .map(|(cell, value)| (binding_index(&indices, cell), value)),
        );
    }

    /// Make room for `additional` in-place bindings. A shared frame is
    /// copied once at its final size instead of at its current one and
    /// then grown binding by binding.
    pub fn reserve(&mut self, additional: usize) {
        if let Some(frame) = &mut self.frame {
            match Rc::get_mut(frame) {
                Some(frame) => frame.bindings.reserve(additional),
                None => {
                    let mut bindings = Vec::with_capacity(frame.bindings.len() + additional);
                    bindings.extend(frame.bindings.iter().cloned());
                    *frame = Rc::new(EnvironmentFrame {
                        parent: frame.parent.clone(),
                        bindings,
                    });
                }
            }
        }
    }

    fn push_indexed(&mut self, bindings: impl IntoIterator<Item = (CellIndex, RuntimeValue)>) {
        let bindings = unboxed(bindings);
        match &mut self.frame {
            Some(frame) => {
                let frame = Rc::make_mut(frame);
                frame.bindings.extend(bindings);
            }
            None => {
                let bindings: Vec<_> = bindings.into_iter().collect();
                if !bindings.is_empty() {
                    self.frame = Some(Rc::new(EnvironmentFrame {
                        parent: None,
                        bindings,
                    }));
                }
            }
        }
    }

    pub(crate) fn extended_indexed(
        &self,
        bindings: impl IntoIterator<Item = (CellIndex, RuntimeValue)>,
    ) -> Self {
        let bindings: Vec<_> = unboxed(bindings).collect();
        if bindings.is_empty() {
            self.clone()
        } else {
            Self {
                indices: self.indices.clone(),
                frame: Some(Rc::new(EnvironmentFrame {
                    parent: self.frame.clone(),
                    bindings,
                })),
            }
        }
    }
}

/// `bindings` with their values unboxed, once as they're bound, so a read
/// needn't look for a number again.
fn unboxed(
    bindings: impl IntoIterator<Item = (CellIndex, RuntimeValue)>,
) -> impl Iterator<Item = (CellIndex, RuntimeValue)> {
    bindings
        .into_iter()
        .map(|(index, value)| (index, unbox(value)))
}

impl From<Environment> for Value {
    fn from(environment: Environment) -> Self {
        Value::from(&environment)
    }
}

impl From<&Environment> for Value {
    fn from(environment: &Environment) -> Self {
        reify::environment(environment)
    }
}
