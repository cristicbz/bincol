use crate::{
    anonymous_union::{serialized_anonymous_variant, UNION_ENUM_NAME},
    described::Described,
    indices::{FieldIndex, FieldListIndex, NameIndex, NameListIndex, SchemaIndex, SchemaListIndex},
    schema::Schema,
    trace::ReadTraceExt,
    trace::TraceNode,
    value::{self, Value},
    RootSchema,
};
use itertools::Itertools;
use serde::{
    ser::{
        Error as _, SerializeMap, SerializeSeq, SerializeTuple, SerializeTupleVariant, Serializer,
    },
    Serialize,
};
use std::{cell::Cell, fmt::Debug};

impl<T> Serialize for Described<T>
where
    T: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value::to_value(&self.0)
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

impl Serialize for Value {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tail = Cell::new(&*self.data.0);
        let cursor = ValueCursor::start(&self.schema, self.root_index, &tail);
        (&self.schema, (self.root_index, cursor)).serialize(serializer)
    }
}

#[derive(Copy, Clone)]
struct ValueCursor<'a> {
    root: &'a RootSchema,
    schema: Schema,
    trace: TraceNode,
    data: &'a [u8],
    tail: &'a Cell<&'a [u8]>,
}

#[derive(Copy, Clone)]
enum CheckResult<'a> {
    Simple,
    Discriminated(u32, ValueCursor<'a>),
}

impl Debug for CheckResult<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Simple => f.debug_struct("Simple").finish(),
            Self::Discriminated(discriminator, cursor) => f
                .debug_struct("Discriminated")
                .field("discriminator", &discriminator)
                .field("schema", &cursor.schema)
                .finish(),
        }
    }
}

impl<'a> ValueCursor<'a> {
    #[inline]
    fn start(root: &'a RootSchema, schema: SchemaIndex, tail: &'a Cell<&'a [u8]>) -> Self {
        Self {
            root,
            schema: root.schema(schema).unwrap(),
            trace: tail.pop_trace_node(),
            tail,
            data: tail.get(),
        }
    }

    #[inline]
    fn pop_child(&self, schema: SchemaIndex) -> Self {
        Self {
            root: self.root,
            schema: self.root.schema(schema).unwrap(),
            trace: self.tail.pop_trace_node(),
            data: self.tail.get(),
            tail: self.tail,
        }
    }

    #[inline]
    fn traced_child(&self, schema: SchemaIndex, trace: TraceNode) -> Self {
        Self {
            root: self.root,
            schema: self.root.schema(schema).unwrap(),
            trace,
            data: self.tail.get(),
            tail: self.tail,
        }
    }

    #[inline]
    fn serialize_inner<S>(&self, serializer: S, inner: SchemaIndex) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.pop_child(inner).serialize(serializer)
    }

    #[inline]
    fn serialize_tuple<S>(
        &self,
        serializer: S,
        schema_length: u32,
        schema_list: SchemaListIndex,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let schema_list = self.root.schema_list(schema_list).unwrap();
        let schema_length = usize::try_from(schema_length).expect("usize at least 32-bits");
        assert_eq!(schema_list.len(), schema_length);

        let mut serializer = serializer.serialize_tuple(schema_length)?;
        for &schema in schema_list {
            serializer.serialize_element(&self.pop_child(schema))?
        }
        serializer.end()
    }

    #[inline]
    fn serialize_map<S>(
        &self,
        serializer: S,
        length: usize,
        key: SchemaIndex,
        value: SchemaIndex,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serializer = serializer.serialize_map(Some(length))?;
        for _ in 0..length {
            serializer.serialize_key(&self.pop_child(key))?;
            serializer.serialize_value(&self.pop_child(value))?;
        }
        serializer.end()
    }

    #[inline]
    fn serialize_sequence<S>(
        &self,
        serializer: S,
        length: usize,
        item: SchemaIndex,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serializer = serializer.serialize_seq(Some(length))?;
        for _ in 0..length {
            serializer.serialize_element(&self.pop_child(item))?;
        }
        serializer.end()
    }

    #[inline]
    fn serialize_struct<S>(
        &self,
        serializer: S,
        name_list: NameListIndex,
        skip_list: FieldListIndex,
        schema_list: SchemaListIndex,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let skip_list = self.root.field_list(skip_list).unwrap();
        let schema_list = self.root.schema_list(schema_list).unwrap();
        let name_list = self.root.name_list(name_list).unwrap();
        let length = self.tail.pop_length_u32();
        let presence = self.tail.pop_slice(length * std::mem::size_of::<u32>());
        assert_eq!(name_list.len(), schema_list.len());

        if skip_list.is_empty() {
            let mut serializer = serializer.serialize_tuple(schema_list.len())?;
            for &schema in schema_list {
                serializer.serialize_element(&self.pop_child(schema))?
            }
            serializer.end()
        } else {
            SkippableStructSerializer {
                cursor: self,
                variant: variant_from_presence(skip_list, presence),
                presence,
                name_list,
                skip_list,
                schema_list,
            }
            .serialize(serializer)
        }
    }

    #[inline]
    fn check(&self) -> Option<CheckResult<'a>> {
        let matches = match (self.trace, self.schema) {
            (TraceNode::Bool, Schema::Bool)
            | (TraceNode::I8, Schema::I8)
            | (TraceNode::I16, Schema::I16)
            | (TraceNode::I32, Schema::I32)
            | (TraceNode::I64, Schema::I64)
            | (TraceNode::I128, Schema::I128)
            | (TraceNode::U8, Schema::U8)
            | (TraceNode::U16, Schema::U16)
            | (TraceNode::U32, Schema::U32)
            | (TraceNode::U64, Schema::U64)
            | (TraceNode::U128, Schema::U128)
            | (TraceNode::F32, Schema::F32)
            | (TraceNode::F64, Schema::F64)
            | (TraceNode::Char, Schema::Char)
            | (TraceNode::String, Schema::String)
            | (TraceNode::Bytes, Schema::Bytes)
            | (TraceNode::None, Schema::OptionNone)
            | (TraceNode::Some, Schema::OptionSome(_))
            | (TraceNode::Unit, Schema::Unit)
            | (TraceNode::Map, Schema::Map(_, _))
            | (TraceNode::Sequence, Schema::Sequence(_)) => true,

            (TraceNode::UnitStruct(trace_name), Schema::UnitStruct(schema_name))
            | (TraceNode::NewtypeStruct(trace_name), Schema::NewtypeStruct(schema_name, _)) => {
                trace_name == schema_name
            }

            (
                TraceNode::UnitVariant(trace_name, trace_variant),
                Schema::UnitVariant(schema_name, schema_variant),
            )
            | (
                TraceNode::NewtypeVariant(trace_name, trace_variant),
                Schema::NewtypeVariant(schema_name, schema_variant, _),
            ) => (trace_name, trace_variant) == (schema_name, schema_variant),

            (TraceNode::Tuple(trace_length), Schema::Tuple(schema_length, _)) => {
                trace_length == schema_length
            }
            (
                TraceNode::TupleStruct(trace_length, trace_name),
                Schema::TupleStruct(schema_name, schema_length, _),
            ) => (trace_length, trace_name) == (schema_length, schema_name),
            (
                TraceNode::TupleVariant(trace_length, trace_name, trace_variant),
                Schema::TupleVariant(schema_name, schema_variant, schema_length, _),
            ) => {
                (trace_length, trace_name, trace_variant)
                    == (schema_length, schema_name, schema_variant)
            }

            (
                TraceNode::Struct(trace_name, trace_name_list),
                Schema::Struct(schema_name, schema_name_list, _, _),
            ) => (trace_name, trace_name_list) == (schema_name, schema_name_list),
            (
                TraceNode::StructVariant(trace_name, trace_variant, trace_name_list),
                Schema::StructVariant(schema_name, schema_variant, schema_name_list, _, _),
            ) => {
                (trace_name, trace_variant, trace_name_list)
                    == (schema_name, schema_variant, schema_name_list)
            }

            (trace, Schema::Union(schema_list)) => {
                return self
                    .root
                    .schema_list(schema_list)
                    .unwrap()
                    .iter()
                    .map(|&schema| self.traced_child(schema, trace))
                    .find_position(|child| child.check().is_some())
                    .map(|(discriminant, child)| {
                        CheckResult::Discriminated(
                            u32::try_from(discriminant).expect("too many types in union"),
                            child,
                        )
                    });
            }

            _ => false,
        };

        matches.then_some(CheckResult::Simple)
    }

    #[inline]
    fn finish_serialize<S>(
        &self,
        serializer: S,
        checked: CheckResult<'_>,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let data = self.tail;
        if let CheckResult::Discriminated(discriminant, child) = checked {
            return serializer.serialize_newtype_variant(
                UNION_ENUM_NAME,
                discriminant,
                serialized_anonymous_variant(discriminant)?,
                &child,
            );
        }
        match self.schema {
            Schema::Bool => serializer.serialize_bool(data.pop_bool()),
            Schema::I8 => serializer.serialize_i8(data.pop_i8()),
            Schema::I16 => serializer.serialize_i16(data.pop_i16()),
            Schema::I32 => serializer.serialize_i32(data.pop_i32()),
            Schema::I64 => serializer.serialize_i64(data.pop_i64()),
            Schema::I128 => serializer.serialize_i128(data.pop_i128()),
            Schema::U8 => serializer.serialize_u8(data.pop_u8()),
            Schema::U16 => serializer.serialize_u16(data.pop_u16()),
            Schema::U32 => serializer.serialize_u32(data.pop_u32()),
            Schema::U64 => serializer.serialize_u64(data.pop_u64()),
            Schema::U128 => serializer.serialize_u128(data.pop_u128()),
            Schema::F32 => serializer.serialize_f32(data.pop_f32()),
            Schema::F64 => serializer.serialize_f64(data.pop_f64()),
            Schema::Char => serializer.serialize_char(data.pop_char()),
            Schema::String => serializer.serialize_str(data.pop_str(data.pop_length_u32())),
            Schema::Bytes => serializer.serialize_bytes(data.pop_slice(data.pop_length_u32())),

            Schema::Unit
            | Schema::UnitStruct(_)
            | Schema::UnitVariant(_, _)
            | Schema::OptionNone => serializer.serialize_unit(),

            Schema::OptionSome(inner)
            | Schema::NewtypeStruct(_, inner)
            | Schema::NewtypeVariant(_, _, inner) => self.serialize_inner(serializer, inner),

            Schema::Map(key, value) => {
                self.serialize_map(serializer, data.pop_length_u32(), key, value)
            }
            Schema::Sequence(item) => {
                self.serialize_sequence(serializer, data.pop_length_u32(), item)
            }

            Schema::Tuple(length, type_list)
            | Schema::TupleStruct(_, length, type_list)
            | Schema::TupleVariant(_, _, length, type_list) => {
                self.serialize_tuple(serializer, length, type_list)
            }

            Schema::Struct(_, name_list, skip_list, type_list)
            | Schema::StructVariant(_, _, name_list, skip_list, type_list) => {
                self.serialize_struct(serializer, name_list, skip_list, type_list)
            }

            Schema::Union(_) => unreachable!("union finish called with simple check result"),
        }
    }
}

struct SkippableStructSerializer<'a, 'v> {
    cursor: &'v ValueCursor<'a>,
    presence: &'a [u8],
    variant: u64,
    name_list: &'a [NameIndex],
    skip_list: &'a [FieldIndex],
    schema_list: &'a [SchemaIndex],
}
impl<'a, 'v> Serialize for SkippableStructSerializer<'a, 'v> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        assert!(self.skip_list.len() <= 64);
        let variant = u32::from(self.variant as u8);
        if self.skip_list.len() <= 8 {
            let mut serializer = serializer.serialize_tuple_variant(
                UNION_ENUM_NAME,
                variant,
                serialized_anonymous_variant(variant)?,
                self.presence.len() / std::mem::size_of::<u32>(),
            )?;
            for field in iter_field_indices(self.presence) {
                serializer.serialize_field(
                    &self.cursor.pop_child(self.schema_list[usize::from(field)]),
                )?;
            }
            serializer.end()
        } else {
            serializer.serialize_newtype_variant(
                UNION_ENUM_NAME,
                variant,
                serialized_anonymous_variant(variant)?,
                &SkippableStructSerializer {
                    cursor: self.cursor,
                    presence: self.presence,
                    variant: self.variant >> 8,
                    name_list: self.name_list,
                    skip_list: &self.skip_list[8..],
                    schema_list: self.schema_list,
                },
            )
        }
    }
}

fn variant_from_presence(skip_list: &[FieldIndex], presence: &[u8]) -> u64 {
    let mut variant = 0u64;
    let mut presence = iter_field_indices(presence).rev().peekable();
    for &skip in skip_list.iter().rev() {
        variant <<= 1;
        while let Some(&present) = presence.peek() {
            if present > skip {
                presence.next();
                continue;
            }
            if present == skip {
                variant |= 1;
                presence.next();
            }
            break;
        }
    }
    variant
}

fn iter_field_indices(presence: &[u8]) -> impl DoubleEndedIterator<Item = FieldIndex> {
    presence
        .chunks_exact(std::mem::size_of::<FieldIndex>())
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .map(|index| FieldIndex::try_from(index).unwrap())
}

impl Serialize for ValueCursor<'_> {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.tail.set(self.data);
        self.finish_serialize(serializer, self.check().expect("schema-trace mismatch"))
    }
}
