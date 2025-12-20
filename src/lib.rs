use indexmap::{Equivalent, IndexSet};
use serde::{
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant,
    },
    Deserialize, Serialize, Serializer,
};
use std::{borrow::Cow, hash::Hash, marker::PhantomData};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ValueKind {
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
    UnitStruct,
    UnitVariant,

    NewtypeStruct,
    NewtypeVariant,

    Sequence,
    Map,
    Tuple,
    TupleStruct,
    TupleVariant,
    Struct,

    Skip,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PartialSchema {
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
    Some(PartialSchemaIndex),

    Unit,
    UnitStruct(PartialNameIndex),
    UnitVariant(PartialNameIndex, PartialNameIndex),

    NewtypeStruct(PartialNameIndex, PartialSchemaIndex),
    NewtypeVariant(PartialNameIndex, PartialNameIndex, PartialSchemaIndex),

    Sequence(PartialSchemaIndex),
    Map(PartialSchemaIndex, PartialSchemaIndex),

    Tuple(Box<[PartialSchemaIndex]>),
    TupleStruct(PartialNameIndex, Box<[PartialSchemaIndex]>),
    TupleVariant(
        PartialNameIndex,
        PartialNameIndex,
        Box<[PartialSchemaIndex]>,
    ),

    Struct(
        PartialNameIndex,
        Box<[(PartialNameIndex, PartialSchemaIndex)]>,
    ),
    StructVariant(
        PartialNameIndex,
        PartialNameIndex,
        Box<[(PartialNameIndex, PartialSchemaIndex)]>,
    ),

    Union(Box<[PartialSchemaIndex]>),
    Skip,
}

pub struct PartialValue([u32; 4]);

macro_rules! u32_indices {
    ($($index_ty:ident => $error:ident,)+) => {
        $(
            #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $index_ty(u32);

            impl From<$index_ty> for u32 {
                fn from(index: $index_ty) -> u32 {
                    index.0.into()
                }
            }

            impl From<$index_ty> for usize {
                fn from(index: $index_ty) -> usize {
                    usize::try_from(u32::from(index.0)).expect("usize must be at least 32-bits")
                }
            }

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
    PartialSchemaIndex => TooManySchemas,
    PartialSchemaListIndex => TooManySchemaLists,
    PartialNameIndex => TooManyNames,
    PartialNameListIndex => TooManyNameLists,
    PartialU32Index => TooManyValues,
    PartialStringIndex => TooManyStrings,
    PartialCharIndex => TooManyStrings,
    PartialByteStringIndex => TooManyByteStrings,
    PartialByteIndex => TooManyByteStrings,
    PartialBytes => TooManyByteStrings,
    PartialValueIndex => TooManyValues,
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct Value {
    bytes_data: Vec<u8>,
    string_data: String,
    value_schemas: Vec<PartialSchemaIndex>,
    value_names: Vec<PartialNameIndex>,
    schemas: Pool<PartialSchema, PartialSchemaIndex>,
    names: Pool<Cow<'static, str>, PartialNameIndex>,
}

impl Value {
    fn push_schema(&mut self, schema: PartialSchema) -> Result<PartialSchemaIndex, SerError> {
        let schema = self.schemas.intern(schema)?;
        self.value_schemas.push(schema);
        Ok(schema)
    }

    fn next_schema(&mut self) -> Result<PartialValueIndex, SerError> {
        PartialValueIndex::try_from(self.value_schemas.len())
    }

    fn reserve_schema(&mut self) -> Result<PartialValueIndex, SerError> {
        let index = self.next_schema()?;
        self.value_schemas.push(PartialSchemaIndex(!0));
        Ok(index)
    }

    fn fill_reserved_schema(
        &mut self,
        index: PartialValueIndex,
        schema: PartialSchema,
    ) -> Result<PartialSchemaIndex, SerError> {
        let schema = self.schemas.intern(schema)?;
        self.value_schemas[usize::from(index)] = schema;
        Ok(schema)
    }

    fn reserve_integer<T>(&mut self) -> Result<PartialByteIndex, SerError> {
        self.reserve_bytes(std::mem::size_of::<T>())
    }

    fn reserve_bytes(&mut self, size: usize) -> Result<PartialByteIndex, SerError> {
        let index = PartialByteIndex::try_from(self.bytes_data.len())?;
        self.bytes_data.extend(std::iter::repeat_n(!0, size));
        Ok(index)
    }

    fn fill_reserved_bytes(&mut self, index: PartialByteIndex, data: &[u8]) {
        self.bytes_data[index.into()..][..data.len()].copy_from_slice(data);
    }
}

pub fn to_value<SerializeT>(value: &SerializeT) -> Result<Value, SerError>
where
    SerializeT: ?Sized + Serialize,
{
    let mut serializer = ValueSerializer::default();
    SerializeT::serialize(value, &mut serializer)?;
    Ok(serializer.value)
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

    pub fn intern_from<FromT>(&mut self, value: FromT) -> Result<ValueIndexT, SerError>
    where
        ValueT: From<FromT>,
    {
        ValueIndexT::try_from(self.inner.insert_full(value.into()).0)
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
    TooManySchemas,

    #[error("too many node lists for u32")]
    TooManySchemaLists,

    #[error("too many structs for u32")]
    TooManyNames,

    #[error("too many field lists for u32")]
    TooManyNameLists,

    #[error("too many strings for u32")]
    TooManyStrings,

    #[error("too many byte strings for u32")]
    TooManyByteStrings,

    #[error("too many values for u32")]
    TooManyValues,

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
    reserved_schema: PartialValueIndex,
    reserved_length: PartialByteIndex,
    schemas: Vec<PartialValueIndex>,
}

impl SerializeSeq for ValueSerializeSeq<'_> {
    type Ok = PartialSchemaIndex;
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let schema = T::serialize(value, &mut *self.parent)?;
        self.schemas.push(schema);
        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.schemas.sort_unstable();
        self.schemas.dedup();
        let item = self
            .parent
            .value
            .schemas
            .intern(PartialSchema::Union(self.schemas.into()))?;
        self.parent
            .value
            .schemas
            .intern(PartialSchema::Sequence(item))
    }
}

pub struct ValueSerializeTuple<'a> {
    parent: &'a mut ValueSerializer,
    schemas: Vec<PartialSchemaIndex>,
}

impl SerializeTuple for ValueSerializeTuple<'_> {
    type Ok = PartialSchemaIndex;
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.schemas.push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.parent
            .value
            .schemas
            .intern(PartialSchema::Tuple(self.schemas.into()))
    }
}

pub struct ValueSerializeTupleStruct<'a> {
    parent: &'a mut ValueSerializer,
    name: PartialNameIndex,
    schemas: Vec<PartialSchemaIndex>,
}

impl SerializeTupleStruct for ValueSerializeTupleStruct<'_> {
    type Ok = PartialSchemaIndex;
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.schemas.push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let node_list = self.parent.value.add_node_list(self.schemas)?;
        self.parent
            .value
            .add_node(ValueNode::TupleStruct(self.name, node_list))
    }
}

pub struct ValueSerializeTupleVariant<'a> {
    parent: &'a mut ValueSerializer,
    name: ValueNameIndex,
    variant: ValueNameIndex,
    schemas: Vec<ValueNodeIndex>,
}

impl SerializeTupleVariant for ValueSerializeTupleVariant<'_> {
    type Ok = PartialSchemaIndex;
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.schemas.push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let node_list = self.parent.value.add_node_list(self.schemas)?;
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
    type Ok = PartialSchemaIndex;
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
    fields: Vec<(ValueNameIndex, ValueNodeIndex)>,
}

impl SerializeStruct for ValueSerializeStruct<'_> {
    type Ok = PartialSchemaIndex;
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let key = self.parent.value.add_name(key)?;
        let value = T::serialize(value, &mut *self.parent)?;
        self.fields.push((key, value));
        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.fields.sort_unstable();
        self.fields.dedup_by_key(|&mut (key, _)| key);
        let (keys, values): (Vec<_>, Vec<_>) = self.fields.into_iter().unzip();
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
    type Ok = PartialSchemaIndex;
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

#[derive(Default, Clone)]
pub struct ValueSerializer {
    value: Value,
}

macro_rules! fn_serialize_as_u8 {
    ($(($fn_name:ident, $value_type:ty, $schema:ident),)+) => {
        $(
            fn $fn_name(self, value: $value_type) -> Result<Self::Ok, Self::Error> {
                self.value.bytes_data.push(value as u8);
                self.value.push_schema(PartialSchema::$schema)
            }
        )+
    };
}

macro_rules! fn_serialize_as_le_bytes {
    ($(($fn_name:ident, $value_type:ty, $schema:ident ),)+) => {
        $(
            fn $fn_name(self, value: $value_type) -> Result<Self::Ok, Self::Error> {
                self.value.bytes_data.extend_from_slice(&value.to_le_bytes());
                self.value.push_schema(PartialSchema::$schema)
            }
        )+
    };
}

impl<'a> Serializer for &'a mut ValueSerializer {
    type Ok = PartialSchemaIndex;
    type Error = SerError;

    type SerializeSeq = ValueSerializeSeq<'a>;
    type SerializeTuple = ValueSerializeTuple<'a>;
    type SerializeTupleStruct = ValueSerializeTupleStruct<'a>;
    type SerializeTupleVariant = ValueSerializeTupleVariant<'a>;
    type SerializeMap = ValueSerializeMap<'a>;
    type SerializeStruct = ValueSerializeStruct<'a>;
    type SerializeStructVariant = ValueSerializeStructVariant<'a>;

    fn_serialize_as_u8! {
        (serialize_bool, bool, Bool),
        (serialize_i8, i8, I8),
        (serialize_u8, u8, U8),
    }

    fn_serialize_as_le_bytes! {
        (serialize_i16, i16, I16),
        (serialize_i32, i32, I32),
        (serialize_i64, i64, I64),
        (serialize_i128, i128, I128),
        (serialize_u16, u16, U16),
        (serialize_u32, u32, U32),
        (serialize_u64, u64, U64),
        (serialize_u128, u128, U128),
        (serialize_f32, f32, F32),
        (serialize_f64, f64, F64),
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.value.string_data.push(value);
        self.value.push_schema(PartialSchema::Char)
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.value
            .bytes_data
            .extend_from_slice(&value.len().to_le_bytes());
        self.value.string_data.push_str(value);
        self.value.push_schema(PartialSchema::String)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.value
            .bytes_data
            .extend_from_slice(&value.len().to_le_bytes());
        self.value.bytes_data.extend_from_slice(value);
        self.value.push_schema(PartialSchema::Bytes)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.value.push_schema(PartialSchema::None)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let reserved = self.value.reserve_schema()?;
        let inner = T::serialize(value, &mut *self)?;
        self.value
            .fill_reserved_schema(reserved, PartialSchema::Some(inner))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.value.push_schema(PartialSchema::Unit)
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        let name = self.value.names.intern_from(name)?;
        self.value.push_schema(PartialSchema::UnitStruct(name))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        let name = self.value.names.intern_from(name)?;
        let variant = self.value.names.intern_from(variant)?;
        self.value
            .push_schema(PartialSchema::UnitVariant(name, variant))
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let name = self.value.names.intern_from(name)?;
        let reserved = self.value.reserve_schema()?;
        let inner = T::serialize(value, &mut *self)?;
        self.value
            .fill_reserved_schema(reserved, PartialSchema::NewtypeStruct(name, inner))
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
        let name = self.value.names.intern_from(name)?;
        let variant = self.value.names.intern_from(variant)?;
        let reserved = self.value.reserve_schema()?;
        let inner = T::serialize(value, &mut *self)?;
        self.value.fill_reserved_schema(
            reserved,
            PartialSchema::NewtypeVariant(name, variant, inner),
        )
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(ValueSerializeSeq {
            reserved_schema: self.value.reserve_schema(),
            reserved_length: self.value.reserve_integer::<usize>(),
            schemas: len.map(Vec::with_capacity).unwrap_or_default(),
            parent: self,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(ValueSerializeTuple {
            parent: self,
            schemas: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        let name = self.value.names.intern_from(name)?;
        Ok(ValueSerializeTupleStruct {
            name,
            parent: self,
            schemas: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        let name = self.value.names.intern_from(name)?;
        let variant = self.value.names.intern_from(variant)?;
        Ok(ValueSerializeTupleVariant {
            name,
            variant,
            parent: self,
            schemas: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(ValueSerializeMap {
            parent: self,
            entries: len.map(Vec::with_capacity).unwrap_or_default(),
            next_key: PartialSchemaIndex(!0),
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        let name = self.value.names.intern_from(name)?;
        Ok(ValueSerializeStruct {
            name,
            parent: self,
            fields: Vec::with_capacity(len),
        })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        let name = self.value.names.intern_from(name)?;
        let variant = self.value.names.intern_from(variant)?;
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
