use indexmap::{Equivalent, IndexSet};
use serde::{
    Serialize, Serializer,
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant,
    },
};
use std::{hash::Hash, marker::PhantomData};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
enum ValueNode {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64([u8; 8]),
    I128([u8; 16]),

    U8(u8),
    U16(u16),
    U32(u32),
    U64([u8; 8]),
    U128([u8; 16]),

    F32(u32),
    F64([u8; 8]),
    Char(char),

    String(ValueStringIndex),
    Bytes(ValueBytesIndex),

    None,
    Some(ValueNodeIndex),

    Unit,
    UnitStruct(ValueNameIndex),
    UnitVariant(ValueNameIndex, ValueNameIndex),

    NewtypeStruct(ValueNameIndex, ValueNodeIndex),
    NewtypeVariant(ValueNameIndex, ValueNameIndex, ValueNodeIndex),

    Sequence(ValueNodeListIndex),
    Map(ValueNodeListIndex, ValueNodeListIndex),

    Tuple(ValueNodeListIndex),
    TupleStruct(ValueNameIndex, ValueNodeListIndex),
    TupleVariant(ValueNameIndex, ValueNameIndex, ValueNodeListIndex),

    Struct(ValueNameIndex, ValueNameListIndex, ValueNodeListIndex),
    StructVariant(
        ValueNameIndex,
        ValueNameIndex,
        ValueNameListIndex,
        ValueNodeListIndex,
    ),
}

macro_rules! u32_indices {
    ($($index_ty:ident => $error:ident,)+) => {
        $(
            #[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $index_ty(u32);

            impl TryFrom<usize> for $index_ty {
                type Error = SerError;

                fn try_from(value: usize) -> Result<Self, Self::Error> {
                    match u32::try_from(value) {
                        Ok(index) => Ok($index_ty(index)),
                        _ => Err(SerError::$error),
                    }
                }
            }
        )+
    };
}

u32_indices! {
    ValueNodeIndex => TooManyNodes,
    ValueNodeListIndex => TooManyNodeLists,
    ValueNameIndex => TooManyNames,
    ValueNameListIndex => TooManyNameLists,
    ValueStringIndex => TooManyStrings,
    ValueBytesIndex => TooManyByteStrings,
}

#[derive(Serialize, Debug)]
pub struct Value {
    root_index: ValueNodeIndex,
    nodes: Pool<ValueNode, ValueNodeIndex>,

    strings: Pool<Box<str>, ValueStringIndex>,
    bytes: Pool<Box<[u8]>, ValueBytesIndex>,
    names: Pool<&'static str, ValueNameIndex>,

    node_lists: Pool<Box<[ValueNodeIndex]>, ValueNodeListIndex>,
    name_lists: Pool<Box<[ValueNameIndex]>, ValueNameListIndex>,
}

impl Value {
    pub fn from_serializable<ValueT>(value: &ValueT) -> Result<Self, SerError>
    where
        ValueT: ?Sized + Serialize,
    {
        let mut serializer = ValueSerializer {
            value: Value {
                root_index: ValueNodeIndex(!0),
                nodes: Pool::new(),

                strings: Pool::new(),
                bytes: Pool::new(),
                names: Pool::new(),

                node_lists: Pool::new(),
                name_lists: Pool::new(),
            },
        };
        let root_index = ValueT::serialize(value, &mut serializer)?;
        serializer.value.root_index = root_index;
        Ok(serializer.value)
    }

    fn add_node(&mut self, node: ValueNode) -> Result<ValueNodeIndex, SerError> {
        self.nodes.intern(node)
    }

    fn add_string(&mut self, string: &str) -> Result<ValueStringIndex, SerError> {
        self.strings.intern_borrowed(string)
    }

    fn add_bytes(&mut self, bytes: &[u8]) -> Result<ValueBytesIndex, SerError> {
        self.bytes.intern_borrowed(bytes)
    }

    fn add_name(&mut self, name: &'static str) -> Result<ValueNameIndex, SerError> {
        self.names.intern_borrowed(name)
    }

    fn add_node_list(
        &mut self,
        node_list: impl Into<Box<[ValueNodeIndex]>>,
    ) -> Result<ValueNodeListIndex, SerError> {
        self.node_lists.intern(node_list.into())
    }

    fn add_name_list(
        &mut self,
        name_list: impl Into<Box<[ValueNameIndex]>>,
    ) -> Result<ValueNameListIndex, SerError> {
        self.name_lists.intern(name_list.into())
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct Pool<ValueT, ValueIndexT> {
    pub inner: IndexSet<ValueT>,

    #[serde(skip)]
    pub _dummy: PhantomData<ValueIndexT>,
}

impl<ValueT, ValueIndexT> Default for Pool<ValueT, ValueIndexT> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            _dummy: PhantomData,
        }
    }
}

impl<ValueT, ValueIndexT> Pool<ValueT, ValueIndexT>
where
    ValueT: Hash + Eq,
    ValueIndexT: TryFrom<usize, Error = SerError>,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, value: ValueT) -> Result<ValueIndexT, SerError> {
        ValueIndexT::try_from(self.inner.insert_full(value).0)
    }

    pub fn intern_borrowed<'s, 'q, QueryT>(
        &'s mut self,
        query: &'q QueryT,
    ) -> Result<ValueIndexT, SerError>
    where
        QueryT: ?Sized + Hash + Equivalent<ValueT>,
        ValueT: From<&'q QueryT>,
    {
        let index = match self.inner.get_full(query) {
            Some((index, _)) => index,
            None => self.inner.insert_full(query.into()).0,
        };
        ValueIndexT::try_from(index)
    }
}

pub fn zigzag64_encode(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

pub fn zigzag64_decode(value: u64) -> i64 {
    (value >> 1) as i64 ^ (-((value & 1) as i64))
}

pub fn zigzag128_encode(value: i128) -> u128 {
    ((value << 1) ^ (value >> 127)) as u128
}

pub fn zigzag128_decode(value: u128) -> i128 {
    (value >> 1) as i128 ^ (-((value & 1) as i128))
}

#[derive(Debug, Error)]
pub enum SerError {
    #[error("too many nodes for u32")]
    TooManyNodes,

    #[error("too many node lists for u32")]
    TooManyNodeLists,

    #[error("too many structs for u32")]
    TooManyNames,

    #[error("too many field lists for u32")]
    TooManyNameLists,

    #[error("too many strings for u32")]
    TooManyStrings,

    #[error("too many byte strings for u32")]
    TooManyByteStrings,

    #[error("attempted to serialize map key without value")]
    UnpairedMapKey,

    #[error("attempted to serialize map value without key")]
    UnpairedMapValue,

    #[error("custom: {0}")]
    Custom(String),
}

impl serde::ser::Error for SerError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        SerError::Custom(msg.to_string())
    }
}

pub struct ValueSerializeSeq<'a> {
    parent: &'a mut ValueSerializer,
    nodes: Vec<ValueNodeIndex>,
}

impl SerializeSeq for ValueSerializeSeq<'_> {
    type Ok = ValueNodeIndex;
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.nodes.push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let node_list = self.parent.value.add_node_list(self.nodes)?;
        self.parent.value.add_node(ValueNode::Sequence(node_list))
    }
}

pub struct ValueSerializeTuple<'a> {
    parent: &'a mut ValueSerializer,
    nodes: Vec<ValueNodeIndex>,
}

impl SerializeTuple for ValueSerializeTuple<'_> {
    type Ok = ValueNodeIndex;
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.nodes.push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let node_list = self.parent.value.add_node_list(self.nodes)?;
        self.parent.value.add_node(ValueNode::Tuple(node_list))
    }
}

pub struct ValueSerializeTupleStruct<'a> {
    parent: &'a mut ValueSerializer,
    name: ValueNameIndex,
    nodes: Vec<ValueNodeIndex>,
}

impl SerializeTupleStruct for ValueSerializeTupleStruct<'_> {
    type Ok = ValueNodeIndex;
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.nodes.push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let node_list = self.parent.value.add_node_list(self.nodes)?;
        self.parent
            .value
            .add_node(ValueNode::TupleStruct(self.name, node_list))
    }
}

pub struct ValueSerializeTupleVariant<'a> {
    parent: &'a mut ValueSerializer,
    name: ValueNameIndex,
    variant: ValueNameIndex,
    nodes: Vec<ValueNodeIndex>,
}

impl SerializeTupleVariant for ValueSerializeTupleVariant<'_> {
    type Ok = ValueNodeIndex;
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.nodes.push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let node_list = self.parent.value.add_node_list(self.nodes)?;
        self.parent
            .value
            .add_node(ValueNode::TupleVariant(self.name, self.variant, node_list))
    }
}

pub struct ValueSerializeMap<'a> {
    parent: &'a mut ValueSerializer,
    entries: Vec<(ValueNodeIndex, ValueNodeIndex)>,
    next_key: ValueNodeIndex,
}

impl SerializeMap for ValueSerializeMap<'_> {
    type Ok = ValueNodeIndex;
    type Error = SerError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        if self.next_key.0 == !0 {
            return Err(SerError::UnpairedMapKey);
        }

        self.next_key = T::serialize(key, &mut *self.parent)?;
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let value = T::serialize(value, &mut *self.parent)?;
        if self.next_key.0 == !0 {
            return Err(SerError::UnpairedMapValue);
        }

        self.entries.push((self.next_key, value));
        self.next_key = ValueNodeIndex(!0);
        Ok(())
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), Self::Error>
    where
        K: ?Sized + Serialize,
        V: ?Sized + Serialize,
    {
        let key = K::serialize(key, &mut *self.parent)?;
        let value = V::serialize(value, &mut *self.parent)?;
        self.entries.push((key, value));
        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.entries.sort_unstable();
        self.entries.dedup_by_key(|&mut (key, _)| key);
        let (keys, values): (Vec<_>, Vec<_>) = self.entries.into_iter().unzip();
        let keys = self.parent.value.add_node_list(keys)?;
        let values = self.parent.value.add_node_list(values)?;
        self.parent.value.add_node(ValueNode::Map(keys, values))
    }
}

pub struct ValueSerializeStruct<'a> {
    parent: &'a mut ValueSerializer,
    name: ValueNameIndex,
    entries: Vec<(ValueNameIndex, ValueNodeIndex)>,
}

impl SerializeStruct for ValueSerializeStruct<'_> {
    type Ok = ValueNodeIndex;
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let key = self.parent.value.add_name(key)?;
        let value = T::serialize(value, &mut *self.parent)?;
        self.entries.push((key, value));
        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.entries.sort_unstable();
        self.entries.dedup_by_key(|&mut (key, _)| key);
        let (keys, values): (Vec<_>, Vec<_>) = self.entries.into_iter().unzip();
        let keys = self.parent.value.add_name_list(keys)?;
        let values = self.parent.value.add_node_list(values)?;
        self.parent
            .value
            .add_node(ValueNode::Struct(self.name, keys, values))
    }
}

pub struct ValueSerializeStructVariant<'a> {
    parent: &'a mut ValueSerializer,
    name: ValueNameIndex,
    variant: ValueNameIndex,
    entries: Vec<(ValueNameIndex, ValueNodeIndex)>,
}

impl SerializeStructVariant for ValueSerializeStructVariant<'_> {
    type Ok = ValueNodeIndex;
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let key = self.parent.value.add_name(key)?;
        let value = T::serialize(value, &mut *self.parent)?;
        self.entries.push((key, value));
        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.entries.sort_unstable();
        self.entries.dedup_by_key(|&mut (key, _)| key);
        let (keys, values): (Vec<_>, Vec<_>) = self.entries.into_iter().unzip();
        let keys = self.parent.value.add_name_list(keys)?;
        let values = self.parent.value.add_node_list(values)?;
        self.parent.value.add_node(ValueNode::StructVariant(
            self.name,
            self.variant,
            keys,
            values,
        ))
    }
}

pub struct ValueSerializer {
    value: Value,
}

impl<'a> Serializer for &'a mut ValueSerializer {
    type Ok = ValueNodeIndex;
    type Error = SerError;

    type SerializeSeq = ValueSerializeSeq<'a>;
    type SerializeTuple = ValueSerializeTuple<'a>;
    type SerializeTupleStruct = ValueSerializeTupleStruct<'a>;
    type SerializeTupleVariant = ValueSerializeTupleVariant<'a>;
    type SerializeMap = ValueSerializeMap<'a>;
    type SerializeStruct = ValueSerializeStruct<'a>;
    type SerializeStructVariant = ValueSerializeStructVariant<'a>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::I8(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::I16(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::I32(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::I64(v.to_le_bytes()))
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::I128(v.to_le_bytes()))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::U8(v))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::U16(v))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::U32(v))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::U64(v.to_le_bytes()))
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::U128(v.to_le_bytes()))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::F32(v.to_bits()))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::F64(v.to_le_bytes()))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::Char(v))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        let node = ValueNode::String(self.value.add_string(v)?);
        self.value.add_node(node)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        let node = ValueNode::Bytes(self.value.add_bytes(v)?);
        self.value.add_node(node)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::None)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let node = T::serialize(value, &mut *self)?;
        self.value.add_node(ValueNode::Some(node))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.value.add_node(ValueNode::Unit)
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        let name = self.value.add_name(name)?;
        self.value.add_node(ValueNode::UnitStruct(name))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        let name = self.value.add_name(name)?;
        let variant = self.value.add_name(variant)?;
        self.value.add_node(ValueNode::UnitVariant(name, variant))
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let name = self.value.add_name(name)?;
        let node = T::serialize(value, &mut *self)?;
        self.value.add_node(ValueNode::NewtypeStruct(name, node))
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let name = self.value.add_name(name)?;
        let variant = self.value.add_name(variant)?;
        let node = T::serialize(value, &mut *self)?;
        self.value
            .add_node(ValueNode::NewtypeVariant(name, variant, node))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(ValueSerializeSeq {
            parent: self,
            nodes: len.map(Vec::with_capacity).unwrap_or_default(),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(ValueSerializeTuple {
            parent: self,
            nodes: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        let name = self.value.add_name(name)?;
        Ok(ValueSerializeTupleStruct {
            name,
            parent: self,
            nodes: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        let name = self.value.add_name(name)?;
        let variant = self.value.add_name(variant)?;
        Ok(ValueSerializeTupleVariant {
            name,
            variant,
            parent: self,
            nodes: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(ValueSerializeMap {
            parent: self,
            entries: len.map(Vec::with_capacity).unwrap_or_default(),
            next_key: ValueNodeIndex(!0),
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        let name = self.value.add_name(name)?;
        Ok(ValueSerializeStruct {
            name,
            parent: self,
            entries: Vec::with_capacity(len),
        })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        let name = self.value.add_name(name)?;
        let variant = self.value.add_name(variant)?;
        Ok(ValueSerializeStructVariant {
            name,
            variant,
            parent: self,
            entries: Vec::with_capacity(len),
        })
    }

    #[inline]
    fn is_human_readable(&self) -> bool {
        false
    }
}

///// A 4-bit encoding method for 64-bit values.
//#[repr(u8)]
//pub enum Encoding {
//    /// 1-bit values.
//    Bits1 = 0,
//
//    /// 2-bit values.
//    Bits2 = 1,
//
//    /// 3-bit values.
//    Bits3 = 2,
//
//    /// 4-bit values.
//    Bits4 = 3,
//
//    /// 5-bit values.
//    Bits5 = 4,
//
//    /// 6-bit values.
//    Bits8 = 5,
//
//    /// 32-bit values.
//    Bits32 = 6,
//
//    /// 64-bit values.
//    Bits64 = 7,
//
//    /// Delta-encoded simple-8b.
//    DeltaSimple8 = 8,
//
//    /// Zig-zag delta-encoded simple-8b.
//    ZigZagDeltaSimple8 = 9,
//
//    /// Delta-of-delta-encoded simple-8b.
//    DeltaOfDeltaSimple8 = 10,
//
//    /// Zig-zag, delta-of-delta-encoded simple-8b.
//    ZigZagDeltaOfDeltaSimple8 = 11,
//
//    /// Varint.
//    Varint = 12,
//
//    /// Delta-encoded varint.
//    DeltaVarint = 13,
//
//    /// Delta-of-delta encoded varint.
//    DeltaOfDeltaVarint = 14,
//
//    /// XOR (for floats).
//    Xor = 15,
//}
//
//#[repr(u8)]
//pub enum VariantKind {
//    Unit = 0,
//    Newtype = 1,
//    Tuple = 2,
//    Struct = 3,
//}
//
//pub enum SchemaKind {
//    // Scalar types
//    // ===============
//    // Take up a `SchemaIndex`, but no values. The only valid `ValueId` is 0.
//    Unit,
//
//    // Take up a `SchemaIndex`, but no values. The 'ValueId'-s are the values themselves.
//    Bool,
//
//    I8,
//    I16,
//
//    U8,
//    U16,
//
//    // The values are stored sorted and deduplicated, delta of delta encoded. `ValueId`-s are indices into this
//    // array.
//    I32,
//    U32,
//
//    I64,
//    U64,
//
//    Char,
//
//    // The values are stored sorted and Xor-encoded, `ValueId`-s are indices into this array.
//    F32,
//    F64,
//
//    // Each value is a length into the `string_pool` and `bytes_pool` respectively. `ValueId`-s are
//    // indices into this length array.
//    String,
//    Bytes,
//
//    // Composite types
//    // ================
//    // Take up `SchemaIndex`, but no values. Uses one entry in `schema_indices` and `ValueId`-s
//    // are offset by +1, with zero corrresponding to `None`.
//    Option,
//
//    // Consumes a `SchemaIndex`, one length per value, and finally, `sum(length)` ValueId-s for the
//    // values.
//    Sequence,
//
//    // Consumes a `SchemaIndex` pointing to the key-value pair (Tuple schema).
//    //
//    // ValueId-s are mapped directly to the key-value pair.
//    Map,
//
//    // An anonymous, ordered, but unique type union (e.g. in a heterogenous collection).
//    //
//    // Consumes a length for the number of types, then that many `SchemaIndex` entries. `ValueId`-s
//    // are offset in turn by each type's number of values.
//    Union,
//
//    /// A tuple (or fixed-size array). Consumes:
//    ///  1. Number of contiguous groups of fields of the same type (K).
//    ///  2. K `SchemaIndex`-s for the contiguous types.
//    ///  3. K lengths for the number of fields of that type. (N1, N2, ... NK).
//    ///
//    /// The total array / tuple length is `N = N1 + N2 + ... + NK`. The values are stored in columnar
//    /// format: all the `ValueId-s` for `field_1`, then all for `field_2`, all the way to
//    /// `field_N`.
//    Tuple,
//
//    // A record is an anonymous struct: it wraps a tuple with field names.
//    //
//    // Consumes a `SchemaIndex` (for the tuple) and as many `FieldNameIndex` as
//    // there are fields in the tuple.
//    //
//    // ValueId-s map directly to the tuple.
//    Record,
//}
//
//#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
//pub struct PartialSchemaIndex(u32);
//
//#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
//pub struct SchemaIndex(u32);
//
//#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
//pub struct FieldNameIndex(u32);
//
//#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
//pub struct PartialFieldNameIndex(u32);
//
//#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
//pub struct PartialValueId(u32);
//
//#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
//pub struct ValueId(u32);
//
//#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
//pub struct FieldName(String);
//
//pub enum InlinePrimitive {
//    Unit,
//    Bool,
//
//    I8,
//    I16,
//
//    U8,
//    U16,
//
//    EmptyOption,
//    EmptySequence,
//    EmptyMap,
//}
//
//pub enum DedupedPrimitive {
//    I32,
//    U32,
//    I64,
//    U64,
//    Char,
//    F32,
//    F64,
//}
//
//pub enum Schema {
//    InlinePrimitive(InlinePrimitive),
//    DedupedPrimitive(DedupedPrimitive),
//    String,
//    Bytes,
//    Option(PartialSchemaIndex),
//    Sequence(PartialSchemaIndex),
//    Map(PartialSchemaIndex),
//    Union(Vec<PartialSchemaIndex>),
//    Tuple(Vec<PartialSchemaIndex>),
//    Record {
//        tuple: PartialSchemaIndex,
//        fields: Vec<PartialFieldNameIndex>,
//    },
//}
//
//#[derive(Default)]
//pub struct PartialFormat {
//    schemas: IndexMap<Schema, IndexSet<PartialValue>>,
//    root: Option<(SchemaIndex, ValueId)>,
//}
//
//enum PartialValue {
//    Empty,
//    Unit,
//    Bool(bool),
//    Number(ron::Number),
//    Char(char),
//    String(String),
//    Bytes(Vec<u8>),
//    List(Vec<PartialValueId>),
//    Other(PartialValueId),
//    Tagged(PartialSchemaIndex, PartialValueId),
//}
//
//impl PartialFormat {
//    pub fn new() -> Self {
//        Self::default()
//    }
//
//    pub fn add(&mut self, value: ron::Value) -> (PartialSchemaIndex, PartialValueId) {
//        match value {
//            ron::Value::Bool(value) => self.schema_value(
//                Schema::InlinePrimitive(InlinePrimitive::Bool),
//                PartialValue::Bool(value),
//            ),
//            ron::Value::Unit => self.schema_value(
//                Schema::InlinePrimitive(InlinePrimitive::Unit),
//                PartialValue::Unit,
//            ),
//            ron::Value::Char(value) => self.schema_value(
//                Schema::DedupedPrimitive(DedupedPrimitive::Char),
//                PartialValue::Char(value),
//            ),
//            ron::Value::String(value) => {
//                self.schema_value(Schema::String, PartialValue::String(value))
//            }
//            ron::Value::Bytes(value) => {
//                self.schema_value(Schema::Bytes, PartialValue::Bytes(value))
//            }
//            ron::Value::Number(number) => {
//                self.schema_value(number_schema(&number), PartialValue::Number(number))
//            }
//            ron::Value::Option(None) => self.schema_value(
//                Schema::InlinePrimitive(InlinePrimitive::EmptyOption),
//                PartialValue::Empty,
//            ),
//            ron::Value::Option(Some(value)) => {
//                let (some_schema, some_value) = self.add(*value);
//                self.schema_value(Schema::Option(some_schema), PartialValue::Other(some_value))
//            }
//            ron::Value::Seq(values) => {
//                let mut schemas = IndexSet::with_capacity(values.len());
//                let mut schema_values = Vec::with_capacity(values.len());
//
//                for value in values {
//                    let schema_value = self.add(value);
//                    schemas.insert(schema_value.0);
//                    schema_values.push(schema_value);
//                }
//
//                match schemas.len() {
//                    0 => self.schema_value(
//                        Schema::InlinePrimitive(InlinePrimitive::EmptySequence),
//                        PartialValue::Empty,
//                    ),
//                    1 => {
//                        let item_schema = schemas.into_iter().next().unwrap();
//                        self.schema_value(
//                            Schema::Sequence(item_schema),
//                            PartialValue::List(
//                                schema_values.into_iter().map(|(_, id)| id).collect(),
//                            ),
//                        )
//                    }
//                    _ => {
//                        schemas.sort_unstable();
//
//                        let item_schema = self.schema(Schema::Union(schemas.into_iter().collect()));
//                        let items = schema_values
//                            .into_iter()
//                            .map(|(item_variant, item_id)| {
//                                self.value(item_schema, PartialValue::Tagged(item_variant, item_id))
//                            })
//                            .collect();
//                        self.schema_value(Schema::Sequence(item_schema), PartialValue::List(items))
//                    }
//                }
//            }
//            ron::Value::Map(values) => todo!(),
//        }
//    }
//
//    fn schema_value(
//        &mut self,
//        schema: Schema,
//        value: PartialValue,
//    ) -> (PartialSchemaIndex, PartialValueId) {
//        todo!()
//    }
//
//    fn schema(&mut self, schema: Schema) -> PartialSchemaIndex {
//        todo!()
//    }
//
//    fn value(&mut self, schema: PartialSchemaIndex, value: PartialValue) -> PartialValueId {
//        todo!()
//    }
//}
//
//fn number_schema(number: &ron::Number) -> Schema {
//    match number {
//        ron::Number::I8(_) => Schema::InlinePrimitive(InlinePrimitive::I8),
//        ron::Number::U8(_) => Schema::InlinePrimitive(InlinePrimitive::I8),
//        ron::Number::I16(_) => Schema::InlinePrimitive(InlinePrimitive::I16),
//        ron::Number::U16(_) => Schema::InlinePrimitive(InlinePrimitive::U16),
//        ron::Number::I32(_) => Schema::DedupedPrimitive(DedupedPrimitive::I32),
//        ron::Number::U32(_) => Schema::DedupedPrimitive(DedupedPrimitive::U32),
//        ron::Number::I64(_) => Schema::DedupedPrimitive(DedupedPrimitive::I64),
//        ron::Number::U64(_) => Schema::DedupedPrimitive(DedupedPrimitive::U64),
//        ron::Number::F32(_) => Schema::DedupedPrimitive(DedupedPrimitive::F32),
//        ron::Number::F64(_) => Schema::DedupedPrimitive(DedupedPrimitive::F64),
//        number => panic!("unknown ron number {number:?}"),
//    }
//}
