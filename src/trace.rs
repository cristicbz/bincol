use serde::Serialize;
use std::{cell::Cell, hash::Hash};

use crate::indices::{FieldNameListIndex, TypeNameIndex, VariantNameIndex};

#[derive(Copy, Debug, Clone)]
pub(crate) enum TraceNode {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,

    U8,
    U16,
    U32,
    U64,
    U128,

    F32,
    F64,
    Char,

    String,
    Bytes,

    None,
    Some,

    Unit,
    UnitStruct(TypeNameIndex),
    UnitVariant(TypeNameIndex, VariantNameIndex),

    NewtypeStruct(TypeNameIndex),
    NewtypeVariant(TypeNameIndex, VariantNameIndex),

    Sequence,
    Map,

    Tuple(u32),
    TupleStruct(u32, TypeNameIndex),
    TupleVariant(u32, TypeNameIndex, VariantNameIndex),

    Struct(TypeNameIndex, FieldNameListIndex),
    StructVariant(TypeNameIndex, VariantNameIndex, FieldNameListIndex),
}

/// Represents a traced serde-serialized value. Returned by
/// [`SchemaBuilder::trace`][`crate::SchemaBuilder::trace`].
///
/// Unlike e.g. `serde_json::Value`, it cannot be used by itself, it must always be used in
/// conjunction with the resulting [`Schema`][`crate::Schema`] returned by the
/// [`SchemaBuilder::build`][`crate::SchemaBuilder::build`] method of the same
/// [`SchemaBuilder`][`crate::SchemaBuilder`] used to produce the value.
#[derive(Default, Clone)]
pub struct Trace(pub(crate) Vec<u8>);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[repr(u8)]
pub enum TraceNodeKind {
    Bool = 0,

    I8,
    I16,
    I32,
    I64,
    I128,

    U8,
    U16,
    U32,
    U64,
    U128,

    F32,
    F64,
    Char,

    String,
    Bytes,

    OptionNone,
    OptionSome,

    Unit,
    UnitStruct,
    UnitVariant,

    NewtypeStruct,
    NewtypeVariant,

    Map,
    Sequence,

    Tuple,
    TupleStruct,
    TupleVariant,

    Struct,
    StructVariant,
}

impl TraceNodeKind {
    const ALL: [Self; 30] = [
        Self::Bool,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::F32,
        Self::F64,
        Self::Char,
        Self::String,
        Self::Bytes,
        Self::OptionNone,
        Self::OptionSome,
        Self::Unit,
        Self::UnitStruct,
        Self::UnitVariant,
        Self::NewtypeStruct,
        Self::NewtypeVariant,
        Self::Map,
        Self::Sequence,
        Self::Tuple,
        Self::TupleStruct,
        Self::TupleVariant,
        Self::Struct,
        Self::StructVariant,
    ];
}

impl TryFrom<u8> for TraceNodeKind {
    type Error = u8;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::ALL.get(usize::from(value)).copied().ok_or(value)
    }
}

impl From<TraceNodeKind> for u8 {
    #[inline]
    fn from(value: TraceNodeKind) -> Self {
        value as Self
    }
}

pub(crate) trait ReadTraceExt<'data> {
    fn pop_u8(&self) -> u8;
    fn pop_slice(&self, len: usize) -> &'data [u8];

    fn pop_str(&self, len: usize) -> &'data str {
        str::from_utf8(self.pop_slice(len)).expect("invalid utf-8 in traced string")
    }

    fn pop_u16(&self) -> u16 {
        u16::from_le_bytes(
            self.pop_slice(std::mem::size_of::<u16>())
                .try_into()
                .expect("impossible"),
        )
    }

    fn pop_u32(&self) -> u32 {
        u32::from_le_bytes(
            self.pop_slice(std::mem::size_of::<u32>())
                .try_into()
                .expect("impossible"),
        )
    }

    fn pop_u64(&self) -> u64 {
        u64::from_le_bytes(
            self.pop_slice(std::mem::size_of::<u64>())
                .try_into()
                .expect("impossible"),
        )
    }

    fn pop_u128(&self) -> u128 {
        u128::from_le_bytes(
            self.pop_slice(std::mem::size_of::<u128>())
                .try_into()
                .expect("impossible"),
        )
    }

    fn pop_trace_node(&self) -> TraceNode {
        let trace = TraceNodeKind::try_from(self.pop_u8()).expect("invalid trace");
        match trace {
            TraceNodeKind::Bool => TraceNode::Bool,
            TraceNodeKind::I8 => TraceNode::I8,
            TraceNodeKind::I16 => TraceNode::I16,
            TraceNodeKind::I32 => TraceNode::I32,
            TraceNodeKind::I64 => TraceNode::I64,
            TraceNodeKind::I128 => TraceNode::I128,
            TraceNodeKind::U8 => TraceNode::U8,
            TraceNodeKind::U16 => TraceNode::U16,
            TraceNodeKind::U32 => TraceNode::U32,
            TraceNodeKind::U64 => TraceNode::U64,
            TraceNodeKind::U128 => TraceNode::U128,
            TraceNodeKind::F32 => TraceNode::F32,
            TraceNodeKind::F64 => TraceNode::F64,
            TraceNodeKind::Char => TraceNode::Char,
            TraceNodeKind::String => TraceNode::String,
            TraceNodeKind::Bytes => TraceNode::Bytes,

            TraceNodeKind::OptionNone => TraceNode::None,
            TraceNodeKind::OptionSome => TraceNode::Some,

            TraceNodeKind::Unit => TraceNode::Unit,

            TraceNodeKind::UnitStruct => TraceNode::UnitStruct(self.pop_type_name()),
            TraceNodeKind::UnitVariant => {
                TraceNode::UnitVariant(self.pop_type_name(), self.pop_variant_name())
            }

            TraceNodeKind::NewtypeStruct => TraceNode::NewtypeStruct(self.pop_type_name()),
            TraceNodeKind::NewtypeVariant => {
                TraceNode::NewtypeVariant(self.pop_type_name(), self.pop_variant_name())
            }

            TraceNodeKind::Map => TraceNode::Map,
            TraceNodeKind::Sequence => TraceNode::Sequence,

            TraceNodeKind::Tuple => TraceNode::Tuple(self.pop_u32()),
            TraceNodeKind::TupleStruct => {
                TraceNode::TupleStruct(self.pop_u32(), self.pop_type_name())
            }
            TraceNodeKind::TupleVariant => TraceNode::TupleVariant(
                self.pop_u32(),
                self.pop_type_name(),
                self.pop_variant_name(),
            ),

            TraceNodeKind::Struct => {
                TraceNode::Struct(self.pop_type_name(), self.pop_field_name_list())
            }
            TraceNodeKind::StructVariant => TraceNode::StructVariant(
                self.pop_type_name(),
                self.pop_variant_name(),
                self.pop_field_name_list(),
            ),
        }
    }

    fn pop_variant_name(&self) -> VariantNameIndex {
        self.pop_u32().into()
    }

    fn pop_type_name(&self) -> TypeNameIndex {
        self.pop_u32().into()
    }

    fn pop_field_name_list(&self) -> FieldNameListIndex {
        self.pop_u32().into()
    }

    fn pop_bool(&self) -> bool {
        self.pop_u8() != 0
    }

    fn pop_i8(&self) -> i8 {
        self.pop_u8() as i8
    }

    fn pop_i16(&self) -> i16 {
        self.pop_u16() as i16
    }

    fn pop_i32(&self) -> i32 {
        self.pop_u32() as i32
    }

    fn pop_i64(&self) -> i64 {
        self.pop_u64() as i64
    }

    fn pop_i128(&self) -> i128 {
        self.pop_u128() as i128
    }

    fn pop_char(&self) -> char {
        char::try_from(self.pop_u32()).expect("expected char")
    }

    fn pop_f32(&self) -> f32 {
        f32::from_bits(self.pop_u32())
    }

    fn pop_f64(&self) -> f64 {
        f64::from_bits(self.pop_u64())
    }

    fn pop_length_u32(&self) -> usize {
        usize::try_from(self.pop_u32()).expect("usize needs to be at least 32 bits")
    }
}

impl<'data> ReadTraceExt<'data> for Cell<&'data [u8]> {
    fn pop_u8(&self) -> u8 {
        let mut data = self.get();
        let byte = *data.split_off_first().expect("expected byte");
        self.set(data);
        byte
    }

    fn pop_slice(&self, len: usize) -> &'data [u8] {
        let (head, tail) = self.get().split_at(len);
        self.set(tail);
        head
    }
}
