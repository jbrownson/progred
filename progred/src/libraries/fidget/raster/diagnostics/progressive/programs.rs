//! Exact whole-program sharing for the retained-renderer experiment only.
//!
//! Keys include both VM tapes and their metadata, not just executable opcodes:
//! subsequent simplification must see the same SSA/choice layout as well.
//! Keep only fingerprints and shared shapes in the table, not another copy of
//! every tape's bytes. Hash matches are confirmed against complete encodings.
use super::*;
use fidget_engine::vm::GenericVmFunction;
use serde::{Serialize, ser::*};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

pub(super) trait Program: Function<Trace = fidget_engine::vm::VmTrace> {
    fn encode(&self, bytes: &mut Vec<u8>);
    fn observe_subtrees(&self, subtrees: &mut super::subtrees::Subtrees);
    fn ssa(&self) -> &fidget_engine::compiler::SsaTape;
    fn from_ssa(
        ssa: fidget_engine::compiler::SsaTape,
        vars: std::sync::Arc<fidget_engine::var::VarMap>,
    ) -> Self;
}

impl<const N: usize> Program for GenericVmFunction<N> {
    fn ssa(&self) -> &fidget_engine::compiler::SsaTape {
        self.data().ssa()
    }
    fn from_ssa(
        ssa: fidget_engine::compiler::SsaTape,
        vars: std::sync::Arc<fidget_engine::var::VarMap>,
    ) -> Self {
        fidget_engine::vm::VmData::<N>::from_ssa(ssa, vars).into()
    }
    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.clear();
        self.data().serialize(&mut Encoding(bytes)).unwrap();
    }

    fn observe_subtrees(&self, subtrees: &mut super::subtrees::Subtrees) {
        subtrees.observe(self.data());
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl Program for fidget_engine::jit::JitFunction {
    fn ssa(&self) -> &fidget_engine::compiler::SsaTape {
        let vm: &GenericVmFunction<_> = self.into();
        vm.ssa()
    }
    fn from_ssa(
        ssa: fidget_engine::compiler::SsaTape,
        vars: std::sync::Arc<fidget_engine::var::VarMap>,
    ) -> Self {
        GenericVmFunction::from(fidget_engine::vm::VmData::from_ssa(ssa, vars)).into()
    }
    fn encode(&self, bytes: &mut Vec<u8>) {
        let vm: &GenericVmFunction<_> = self.into();
        vm.encode(bytes);
    }

    fn observe_subtrees(&self, subtrees: &mut super::subtrees::Subtrees) {
        let vm: &GenericVmFunction<_> = self.into();
        vm.observe_subtrees(subtrees);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Statistics {
    pub candidates: usize,
    pub candidate_ops: usize,
    pub duplicates: usize,
    pub duplicate_ops: usize,
}

struct Entries<F> {
    shapes: HashMap<u64, Vec<Shape<F>>>,
    statistics: Statistics,
}

pub(super) struct Programs<F> {
    entries: Mutex<Entries<F>>,
    share: bool,
}

impl<F: Program> Programs<F> {
    pub fn new(share: bool) -> Self {
        Self {
            entries: Mutex::new(Entries {
                shapes: HashMap::new(),
                statistics: Statistics::default(),
            }),
            share,
        }
    }

    pub fn intern(
        &self,
        shape: Shape<F>,
        scratch: &mut [Vec<u8>; 2],
        storage: &mut Vec<F::Storage>,
    ) -> Shape<F> {
        shape.inner().encode(&mut scratch[0]);
        let mut hash = DefaultHasher::new();
        scratch[0].hash(&mut hash);
        self.intern_hashed(hash.finish(), shape, scratch, storage)
    }

    fn intern_hashed(
        &self,
        hash: u64,
        shape: Shape<F>,
        scratch: &mut [Vec<u8>; 2],
        storage: &mut Vec<F::Storage>,
    ) -> Shape<F> {
        let mut entries = self.entries.lock().unwrap();
        entries.statistics.candidates += 1;
        entries.statistics.candidate_ops += shape.size();
        let bucket = entries.shapes.entry(hash).or_default();
        for candidate in bucket.iter() {
            candidate.inner().encode(&mut scratch[1]);
            if scratch[0] == scratch[1] {
                let shared = candidate.clone();
                entries.statistics.duplicates += 1;
                entries.statistics.duplicate_ops += shape.size();
                if self.share {
                    storage.extend(shape.recycle());
                    return shared;
                }
                return shape;
            }
        }
        bucket.push(shape.clone());
        shape
    }

    pub fn statistics(&self) -> Statistics {
        self.entries.lock().unwrap().statistics
    }
}

// A small diagnostic encoding over Fidget's existing Serialize implementation.
// Unlike JSON it preserves infinities, NaN payloads, and signed zero. This is
// not a file format. It is used only to compare values of the same VmData type.
// Maps sort their encoded entries so HashMap iteration order is irrelevant.
struct Encoding<'a>(&'a mut Vec<u8>);
type Error = serde::de::value::Error;

macro_rules! integer {
    ($($method:ident($ty:ty)),* $(,)?) => {$(
        fn $method(self, value: $ty) -> Result<(), Error> {
            self.0.extend_from_slice(&value.to_le_bytes()); Ok(())
        }
    )*};
}

impl<'a, 'b> Serializer for &'a mut Encoding<'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Map<'a, 'b>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    integer!(
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128)
    );
    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.serialize_u8(v.into())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        self.serialize_u32(v.to_bits())
    }
    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        self.serialize_u64(v.to_bits())
    }
    fn serialize_char(self, v: char) -> Result<(), Error> {
        self.serialize_u32(v as u32)
    }
    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.serialize_bytes(v.as_bytes())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.serialize_u64(v.len() as u64)?;
        self.0.extend_from_slice(v);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Error> {
        self.serialize_u8(0)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<(), Error> {
        self.serialize_u8(1)?;
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> {
        Ok(())
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        index: u32,
        _: &'static str,
    ) -> Result<(), Error> {
        self.serialize_u32(index)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        v: &T,
    ) -> Result<(), Error> {
        v.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        index: u32,
        _: &'static str,
        v: &T,
    ) -> Result<(), Error> {
        self.serialize_u32(index)?;
        v.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self, Error> {
        self.serialize_u64(
            len.ok_or_else(|| {
                <Error as serde::ser::Error>::custom("expected known sequence length")
            })? as u64,
        )?;
        Ok(self)
    }
    fn serialize_tuple(self, len: usize) -> Result<Self, Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Self, Error> {
        self.serialize_tuple(len)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        index: u32,
        _: &'static str,
        len: usize,
    ) -> Result<Self, Error> {
        self.serialize_u32(index)?;
        self.serialize_tuple(len)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Ok(Map {
            parent: self,
            entries: vec![],
            key: None,
        })
    }
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<Self, Error> {
        self.serialize_tuple(len)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        index: u32,
        _: &'static str,
        len: usize,
    ) -> Result<Self, Error> {
        self.serialize_u32(index)?;
        self.serialize_tuple(len)
    }
    fn is_human_readable(&self) -> bool {
        false
    }
}

macro_rules! sequence {
    ($($trait:ident::$method:ident),*) => {$(
        impl $trait for &mut Encoding<'_> {
            type Ok = (); type Error = Error;
            fn $method<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Error> { v.serialize(&mut **self) }
            fn end(self) -> Result<(), Error> { Ok(()) }
        }
    )*};
}
sequence!(
    SerializeSeq::serialize_element,
    SerializeTuple::serialize_element,
    SerializeTupleStruct::serialize_field,
    SerializeTupleVariant::serialize_field
);

macro_rules! record {
    ($($trait:ident),*) => {$(
        impl $trait for &mut Encoding<'_> {
            type Ok = (); type Error = Error;
            fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, v: &T) -> Result<(), Error> {
                key.serialize(&mut **self)?; v.serialize(&mut **self)
            }
            fn end(self) -> Result<(), Error> { Ok(()) }
        }
    )*};
}
record!(SerializeStruct, SerializeStructVariant);

struct Map<'a, 'b> {
    parent: &'a mut Encoding<'b>,
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    key: Option<Vec<u8>>,
}
impl SerializeMap for Map<'_, '_> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Error> {
        let mut bytes = vec![];
        key.serialize(&mut Encoding(&mut bytes))?;
        self.key = Some(bytes);
        Ok(())
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        let mut bytes = vec![];
        value.serialize(&mut Encoding(&mut bytes))?;
        self.entries
            .push((self.key.take().expect("map key before value"), bytes));
        Ok(())
    }
    fn end(mut self) -> Result<(), Error> {
        self.entries.sort();
        self.parent.serialize_u64(self.entries.len() as u64)?;
        for (key, value) in self.entries {
            self.parent.serialize_bytes(&key)?;
            self.parent.serialize_bytes(&value)?;
        }
        Ok(())
    }
}

#[test]
fn retained_program_encoding_preserves_bits_and_map_identity() {
    let encode = |v: f32| {
        let mut out = vec![];
        v.serialize(&mut Encoding(&mut out)).unwrap();
        out
    };
    for (a, b) in [
        (0.0, -0.0),
        (f32::INFINITY, f32::NEG_INFINITY),
        (f32::from_bits(0x7fc00001), f32::from_bits(0x7fc00002)),
    ] {
        assert_ne!(encode(a), encode(b));
    }
    let a: HashMap<_, _> = [(1u32, 2u32), (3, 4)].into_iter().collect();
    let b: HashMap<_, _> = [(3u32, 4u32), (1, 2)].into_iter().collect();
    let mut x = vec![];
    let mut y = vec![];
    a.serialize(&mut Encoding(&mut x)).unwrap();
    b.serialize(&mut Encoding(&mut y)).unwrap();
    assert_eq!(x, y);
}

#[test]
fn retained_program_sharing_confirms_hash_matches() {
    use fidget_engine::{var::Var, vm::VmShape};
    let pool = Programs::new(true);
    let mut scratch: [Vec<u8>; 2] = Default::default();
    let mut storage = vec![];
    let mut intern = |tree| {
        let shape = VmShape::from(tree);
        shape.inner().encode(&mut scratch[0]);
        pool.intern_hashed(0, shape, &mut scratch, &mut storage)
    };
    let a = intern(Tree::x() + 1.0);
    let b = intern(Tree::x() + 2.0);
    let c = intern(Tree::x() + 1.0);
    // Identical arithmetic over different variables is not the same program.
    let d = intern(Tree::from(Var::new()) + 1.0);
    let e = intern(Tree::from(Var::new()) + 1.0);
    assert!(!std::ptr::eq(a.inner().data(), b.inner().data()));
    assert!(std::ptr::eq(a.inner().data(), c.inner().data()));
    assert!(!std::ptr::eq(d.inner().data(), e.inner().data()));
    assert_eq!(pool.statistics().duplicates, 1);
}
