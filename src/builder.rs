use crate::{
    errors::SerError,
    indices::{
        FieldIndex, FieldListIndex, NameIndex, NameListIndex, SchemaIndex, SchemaListIndex,
        TraceIndex, TypeName,
    },
    pool::Pool,
    schema::{RootSchema, Schema},
    trace::Trace,
    value::{Value, ValueData},
};
use serde::{
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant, Serializer,
    },
    Deserialize, Serialize,
};
use std::fmt::Debug;

#[derive(Default, Clone, Serialize)]
pub(crate) struct RootSchemaBuilder {
    data: Vec<u8>,
    names: Pool<&'static str, NameIndex>,
    name_lists: Pool<Box<[NameIndex]>, NameListIndex>,
    schemas: Pool<Schema, SchemaIndex>,
    schema_lists: Pool<Box<[SchemaIndex]>, SchemaListIndex>,
    field_lists: Pool<Box<[FieldIndex]>, FieldListIndex>,
    root: SchemaBuilder,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum SchemaBuilder {
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

    OptionNone,
    OptionSome(Box<SchemaBuilder>),

    Unit(Option<TypeName>),
    Newtype(TypeName, Box<SchemaBuilder>),

    Map(Box<SchemaBuilder>, Box<SchemaBuilder>),
    Sequence(Box<SchemaBuilder>),

    Union(Vec<SchemaBuilder>),

    /// Tuple, tuple struct, tuple variant, struct or struct variant.
    Record {
        name: Option<TypeName>,
        field_names: Option<NameListIndex>,
        field_types: Vec<SchemaBuilder>,
        skippable: Vec<FieldIndex>,
        length: u32,
    },
}

impl SchemaBuilder {
    fn unify(&mut self, other: Self) -> Result<(), Self> {
        match (&mut *self, other) {
            (SchemaBuilder::Union(lefts), right) => {
                if lefts.is_empty() {
                    *self = right;
                } else {
                    right.add_to_nonempty_union(lefts);
                }
                Ok(())
            }
            (left, mut right @ SchemaBuilder::Union(_)) => {
                std::mem::swap(left, &mut right);
                left.unify(right)
            }
            (
                SchemaBuilder::Newtype(left_name, left_inner),
                SchemaBuilder::Newtype(right_name, right_inner),
            ) => {
                if *left_name == right_name {
                    left_inner.union(*right_inner);
                    Ok(())
                } else {
                    Err(SchemaBuilder::Newtype(right_name, right_inner))
                }
            }
            (SchemaBuilder::OptionSome(left), SchemaBuilder::OptionSome(right)) => {
                left.union(*right);
                Ok(())
            }
            (
                SchemaBuilder::Record {
                    name: left_name,
                    field_names: left_field_names,
                    field_types: left_field_types,
                    skippable: left_skippable,
                    length: left_length,
                },
                SchemaBuilder::Record {
                    name: right_name,
                    field_names: right_field_names,
                    field_types: right_field_types,
                    skippable: right_skippable,
                    length: right_length,
                },
            ) => {
                if (*left_name, *left_field_names, *left_length)
                    == (right_name, right_field_names, right_length)
                {
                    left_field_types
                        .iter_mut()
                        .zip(right_field_types)
                        .for_each(|(left, right)| left.union(right));
                    left_skippable.extend(right_skippable);
                    left_skippable.sort_unstable();
                    left_skippable.dedup();
                    Ok(())
                } else {
                    Err(SchemaBuilder::Record {
                        name: right_name,
                        field_names: right_field_names,
                        field_types: right_field_types,
                        skippable: right_skippable,
                        length: right_length,
                    })
                }
            }
            (
                SchemaBuilder::Map(left_keys, left_values),
                SchemaBuilder::Map(right_keys, right_values),
            ) => {
                left_keys.union(*right_keys);
                left_values.union(*right_values);
                Ok(())
            }
            (SchemaBuilder::Sequence(left), SchemaBuilder::Sequence(right)) => {
                left.union(*right);
                Ok(())
            }
            (left, right) => {
                if *left == right {
                    Ok(())
                } else {
                    Err(right)
                }
            }
        }
    }

    #[inline]
    fn union(&mut self, other: Self) {
        if let Err(other) = self.unify(other) {
            let left = std::mem::take(self);
            match self {
                SchemaBuilder::Union(schemas) => *schemas = vec![left, other],
                _ => unreachable!(),
            }
        }
    }

    fn add_to_nonempty_union(self, lefts: &mut Vec<SchemaBuilder>) {
        assert!(!lefts.is_empty());
        match self {
            SchemaBuilder::Union(rights) => {
                rights
                    .into_iter()
                    .for_each(|right| right.add_to_nonempty_union(lefts));
            }
            right => {
                let right = lefts
                    .iter_mut()
                    .try_fold(right, |right, left| match left.unify(right) {
                        Ok(()) => Err(()),
                        Err(recovered) => Ok(recovered),
                    })
                    .ok();
                lefts.extend(right);
            }
        }
    }
}

impl Default for SchemaBuilder {
    #[inline]
    fn default() -> Self {
        Self::Union(Vec::new())
    }
}

impl SchemaBuilder {
    fn build(self, root: &mut RootSchemaBuilder) -> Result<SchemaIndex, SerError> {
        let schema = match self {
            SchemaBuilder::Bool => Schema::Bool,
            SchemaBuilder::I8 => Schema::I8,
            SchemaBuilder::I16 => Schema::I16,
            SchemaBuilder::I32 => Schema::I32,
            SchemaBuilder::I64 => Schema::I64,
            SchemaBuilder::I128 => Schema::I128,

            SchemaBuilder::U8 => Schema::U8,
            SchemaBuilder::U16 => Schema::U16,
            SchemaBuilder::U32 => Schema::U32,
            SchemaBuilder::U64 => Schema::U64,
            SchemaBuilder::U128 => Schema::U128,

            SchemaBuilder::F32 => Schema::F32,
            SchemaBuilder::F64 => Schema::F64,
            SchemaBuilder::Char => Schema::Char,

            SchemaBuilder::String => Schema::String,
            SchemaBuilder::Bytes => Schema::Bytes,

            SchemaBuilder::OptionNone => Schema::OptionNone,
            SchemaBuilder::OptionSome(inner) => {
                let inner = inner.build(root)?;
                Schema::OptionSome(inner)
            }
            SchemaBuilder::Unit(None) => Schema::Unit,
            SchemaBuilder::Unit(Some(TypeName(name, None))) => Schema::UnitStruct(name),
            SchemaBuilder::Unit(Some(TypeName(name, Some(variant)))) => {
                Schema::UnitVariant(name, variant)
            }
            SchemaBuilder::Newtype(type_name, inner) => {
                let inner = inner.build(root)?;
                match type_name {
                    TypeName(name, None) => Schema::NewtypeStruct(name, inner),
                    TypeName(name, Some(variant)) => Schema::NewtypeVariant(name, variant, inner),
                }
            }
            SchemaBuilder::Map(key, value) => Schema::Map(key.build(root)?, value.build(root)?),
            SchemaBuilder::Sequence(item) => Schema::Sequence(item.build(root)?),
            SchemaBuilder::Union(schemas) => {
                let mut schemas = schemas
                    .into_iter()
                    .map(|schema| schema.build(root))
                    .collect::<Result<Vec<_>, _>>()?;
                schemas.sort_unstable();
                schemas.dedup();
                Schema::Union(root.schema_lists.intern_from(schemas)?)
            }
            SchemaBuilder::Record {
                name,
                field_names,
                field_types,
                length,
                mut skippable,
            } => {
                skippable.retain(|&index| {
                    !matches!(
                        &field_types[usize::from(index)],
                        SchemaBuilder::Union(variants) if variants.is_empty()
                    )
                });
                let field_types = field_types
                    .into_iter()
                    .map(|schema| schema.build(root))
                    .collect::<Result<Vec<_>, _>>()?;
                let field_types = root.schema_lists.intern_from(field_types)?;
                match (name, field_names) {
                    (None, None) => Schema::Tuple(length, field_types),
                    (Some(TypeName(name, None)), None) => {
                        Schema::TupleStruct(name, length, field_types)
                    }
                    (Some(TypeName(name, Some(variant))), None) => {
                        Schema::TupleVariant(name, variant, length, field_types)
                    }
                    (None, Some(_field_names)) => {
                        unreachable!("anonymous structs don't exist in rust!")
                    }
                    (Some(TypeName(name, None)), Some(field_names)) => {
                        let skip_list = root.field_lists.intern_from(skippable)?;
                        Schema::Struct(name, field_names, skip_list, field_types)
                    }
                    (Some(TypeName(name, Some(variant))), Some(field_names)) => {
                        let skip_list = root.field_lists.intern_from(skippable)?;
                        Schema::StructVariant(name, variant, field_names, skip_list, field_types)
                    }
                }
            }
        };
        root.schemas.intern(schema)
    }
}

impl RootSchemaBuilder {
    pub(crate) fn new<ValueT>(value: &ValueT) -> Result<Self, SerError>
    where
        ValueT: ?Sized + Serialize,
    {
        let mut this = Self::default();
        let root = ValueT::serialize(value, &mut this)?;
        this.root = root;
        Ok(this)
    }

    pub(crate) fn into_value(mut self) -> Result<Value, SerError> {
        let root = std::mem::take(&mut self.root);
        let root_index = root.build(&mut self)?;
        let schema = RootSchema {
            schemas: self.schemas.into_iter().collect::<Vec<_>>().into(),
            names: self
                .names
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into(),
            name_lists: self.name_lists.into_iter().collect::<Vec<_>>().into(),
            schema_lists: self.schema_lists.into_iter().collect::<Vec<_>>().into(),
            field_lists: self.field_lists.into_iter().collect::<Vec<_>>().into(),
        };
        schema.dump(&mut String::new(), root_index).unwrap();
        Ok(Value {
            schema,
            root_index,
            data: ValueData(self.data),
        })
    }

    #[inline]
    fn push_struct_name(&mut self, name: &'static str) -> Result<TypeName, SerError> {
        let name = self.names.intern(name)?;
        self.push_u32(name.into());
        Ok(TypeName(name, None))
    }

    #[inline]
    fn push_variant_name(
        &mut self,
        name: &'static str,
        variant: &'static str,
    ) -> Result<TypeName, SerError> {
        let name = self.names.intern(name)?;
        let variant = self.names.intern(variant)?;
        self.push_u32(name.into());
        self.push_u32(variant.into());
        Ok(TypeName(name, Some(variant)))
    }

    #[inline]
    fn intern_field_name(&mut self, name: &'static str) -> Result<NameIndex, SerError> {
        self.names.intern(name)
    }

    #[inline]
    fn fill_reserved_field_name_list<NameListT>(
        &mut self,
        index: TraceIndex,
        names: NameListT,
    ) -> Result<NameListIndex, SerError>
    where
        Box<[NameIndex]>: From<NameListT>,
    {
        let names = self.name_lists.intern_from(names)?;
        self.fill_reserved_bytes(index, &u32::from(names).to_le_bytes());
        Ok(names)
    }

    #[inline]
    fn push_u32(&mut self, integer: u32) {
        self.data.extend(integer.to_le_bytes());
    }

    #[inline]
    fn push_u32_length(&mut self, length: usize) -> Result<(), SerError> {
        self.data.extend(
            u32::try_from(length)
                .map_err(|_| SerError::TooManyValues)?
                .to_le_bytes(),
        );
        Ok(())
    }

    #[inline]
    fn push_trace(&mut self, trace: Trace) {
        self.data.push(trace.into());
    }

    #[inline]
    fn reserve_u32(&mut self) -> Result<TraceIndex, SerError> {
        self.reserve_bytes(std::mem::size_of::<u32>())
    }

    #[inline]
    fn reserve_field_presence(&mut self, length: usize) -> Result<TraceIndex, SerError> {
        self.reserve_bytes(std::mem::size_of::<u32>() * length)
    }

    #[inline]
    fn reserve_bytes(&mut self, size: usize) -> Result<TraceIndex, SerError> {
        let index = TraceIndex::try_from(self.data.len())?;
        self.data.extend(std::iter::repeat_n(!0, size));
        Ok(index)
    }

    #[inline]
    fn push_length_bytes(&mut self, bytes: &[u8]) -> Result<(), SerError> {
        self.push_u32_length(bytes.len())?;
        self.data.extend(bytes);
        Ok(())
    }

    #[inline]
    fn fill_reserved_bytes(&mut self, index: TraceIndex, data: &[u8]) {
        self.data[index.into()..][..data.len()].copy_from_slice(data);
    }

    #[inline]
    fn write_field_presence(
        &mut self,
        index: TraceIndex,
        field: FieldIndex,
    ) -> Result<TraceIndex, SerError> {
        self.fill_reserved_bytes(index, &u32::from(field).to_le_bytes());
        TraceIndex::try_from(usize::from(index) + std::mem::size_of::<u32>())
    }
}

macro_rules! fn_serialize_as_u8 {
    ($(($fn_name:ident, $value_type:ty, $schema:ident),)+) => {
        $(
            #[inline]
            fn $fn_name(self, value: $value_type) -> Result<Self::Ok, Self::Error> {
                self.push_trace(Trace::$schema);
                self.data.push(value as u8);
                Ok(SchemaBuilder::$schema)
            }
        )+
    };
}

macro_rules! fn_serialize_as_le_bytes {
    ($(($fn_name:ident, $value_type:ty, $schema:ident ),)+) => {
        $(
            #[inline]
            fn $fn_name(self, value: $value_type) -> Result<Self::Ok, Self::Error> {

                self.push_trace(Trace::$schema);
                self.data.extend_from_slice(&value.to_le_bytes());
                Ok(SchemaBuilder::$schema)
            }
        )+
    };
}

impl<'a> Serializer for &'a mut RootSchemaBuilder {
    type Ok = SchemaBuilder;
    type Error = SerError;

    type SerializeSeq = SequenceSchemaBuilder<'a>;
    type SerializeTuple = TupleSchemaBuilder<'a>;
    type SerializeTupleStruct = TupleSchemaBuilder<'a>;
    type SerializeTupleVariant = TupleSchemaBuilder<'a>;
    type SerializeMap = MapSchemaBuilder<'a>;
    type SerializeStruct = StructSchemaBuilder<'a>;
    type SerializeStructVariant = StructSchemaBuilder<'a>;

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

    #[inline]
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.push_trace(Trace::Char);
        self.push_u32(u32::from(value));
        Ok(SchemaBuilder::Char)
    }

    #[inline]
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.push_trace(Trace::String);
        self.push_length_bytes(value.as_bytes())?;
        Ok(SchemaBuilder::String)
    }

    #[inline]
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.push_trace(Trace::Bytes);
        self.push_length_bytes(value)?;
        Ok(SchemaBuilder::Bytes)
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.push_trace(Trace::OptionNone);
        Ok(SchemaBuilder::OptionNone)
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.push_trace(Trace::OptionSome);
        T::serialize(value, &mut *self).map(|schema| SchemaBuilder::OptionSome(Box::new(schema)))
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.push_trace(Trace::Unit);
        Ok(SchemaBuilder::Unit(None))
    }

    #[inline]
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.push_trace(Trace::UnitStruct);
        Ok(SchemaBuilder::Unit(Some(self.push_struct_name(name)?)))
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.push_trace(Trace::UnitVariant);
        Ok(SchemaBuilder::Unit(Some(
            self.push_variant_name(name, variant)?,
        )))
    }

    #[inline]
    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.push_trace(Trace::NewtypeStruct);
        Ok(SchemaBuilder::Newtype(
            self.push_struct_name(name)?,
            Box::new(T::serialize(value, &mut *self)?),
        ))
    }

    #[inline]
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
        self.push_trace(Trace::NewtypeVariant);
        Ok(SchemaBuilder::Newtype(
            self.push_variant_name(name, variant)?,
            Box::new(T::serialize(value, &mut *self)?),
        ))
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.push_trace(Trace::Sequence);
        Ok(SequenceSchemaBuilder {
            reserved_length: self.reserve_u32()?,
            schema: SchemaBuilder::default(),
            length: 0,
            parent: self,
        })
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.push_trace(Trace::Tuple);
        self.push_u32_length(len)?;
        Ok(TupleSchemaBuilder {
            name: None,
            schemas: Vec::with_capacity(len),
            parent: self,
            length: u32::try_from(len).map_err(|_| SerError::TooManyValues)?,
        })
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.push_trace(Trace::TupleStruct);
        self.push_u32_length(len)?;
        Ok(TupleSchemaBuilder {
            name: Some(self.push_struct_name(name)?),
            schemas: Vec::with_capacity(len),
            parent: self,
            length: u32::try_from(len).map_err(|_| SerError::TooManyValues)?,
        })
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.push_trace(Trace::TupleVariant);
        self.push_u32_length(len)?;
        Ok(TupleSchemaBuilder {
            name: Some(self.push_variant_name(name, variant)?),
            schemas: Vec::with_capacity(len),
            parent: self,
            length: u32::try_from(len).map_err(|_| SerError::TooManyValues)?,
        })
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.push_trace(Trace::Map);
        Ok(MapSchemaBuilder {
            reserved_length: self.reserve_u32()?,
            key_schema: SchemaBuilder::default(),
            value_schema: SchemaBuilder::default(),
            length: 0,
            parent: self,
        })
    }

    #[inline]
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.push_trace(Trace::Struct);
        StructSchemaBuilder::new(self.push_struct_name(name)?, len, self)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.push_trace(Trace::StructVariant);
        StructSchemaBuilder::new(self.push_variant_name(name, variant)?, len, self)
    }

    #[inline]
    fn is_human_readable(&self) -> bool {
        false
    }
}

pub(crate) struct SequenceSchemaBuilder<'a> {
    parent: &'a mut RootSchemaBuilder,
    reserved_length: TraceIndex,
    schema: SchemaBuilder,
    length: u32,
}

impl SerializeSeq for SequenceSchemaBuilder<'_> {
    type Ok = SchemaBuilder;
    type Error = SerError;

    #[inline]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.length += 1;
        self.schema.union(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.parent
            .fill_reserved_bytes(self.reserved_length, &self.length.to_le_bytes());
        Ok(SchemaBuilder::Sequence(Box::new(self.schema)))
    }
}

pub(crate) struct MapSchemaBuilder<'a> {
    parent: &'a mut RootSchemaBuilder,
    reserved_length: TraceIndex,
    key_schema: SchemaBuilder,
    value_schema: SchemaBuilder,
    length: u32,
}

impl SerializeMap for MapSchemaBuilder<'_> {
    type Ok = SchemaBuilder;
    type Error = SerError;

    #[inline]
    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.length += 1;
        self.key_schema.union(T::serialize(key, &mut *self.parent)?);
        Ok(())
    }

    #[inline]
    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.value_schema
            .union(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.parent
            .fill_reserved_bytes(self.reserved_length, &self.length.to_le_bytes());
        Ok(SchemaBuilder::Map(
            Box::new(self.key_schema),
            Box::new(self.value_schema),
        ))
    }
}

pub(crate) struct TupleSchemaBuilder<'a> {
    name: Option<TypeName>,
    schemas: Vec<SchemaBuilder>,
    parent: &'a mut RootSchemaBuilder,
    length: u32,
}

impl SerializeTuple for TupleSchemaBuilder<'_> {
    type Ok = SchemaBuilder;
    type Error = SerError;

    #[inline]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.schemas.push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(SchemaBuilder::Record {
            name: self.name,
            field_names: None,
            field_types: self.schemas,
            length: self.length,
            skippable: Vec::new(),
        })
    }
}

impl SerializeTupleStruct for TupleSchemaBuilder<'_> {
    type Ok = SchemaBuilder;
    type Error = SerError;

    #[inline]
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        <Self as SerializeTuple>::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as SerializeTuple>::end(self)
    }
}

impl SerializeTupleVariant for TupleSchemaBuilder<'_> {
    type Ok = SchemaBuilder;
    type Error = SerError;

    #[inline]
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        <Self as SerializeTuple>::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as SerializeTuple>::end(self)
    }
}

pub(crate) struct StructSchemaBuilder<'a> {
    name: TypeName,
    reserved_field_name_list: TraceIndex,
    reserved_field_presence: TraceIndex,
    field_names: Vec<NameIndex>,
    field_types: Vec<SchemaBuilder>,
    skipped: Vec<FieldIndex>,
    parent: &'a mut RootSchemaBuilder,
}

impl<'a> StructSchemaBuilder<'a> {
    pub fn new(
        name: TypeName,
        length: usize,
        parent: &'a mut RootSchemaBuilder,
    ) -> Result<Self, SerError> {
        let reserved_field_name_list = parent.reserve_u32()?;
        parent.push_u32_length(length)?;
        Ok(Self {
            name,
            reserved_field_name_list,
            reserved_field_presence: parent.reserve_field_presence(length)?,
            field_names: Vec::with_capacity(length),
            field_types: Vec::with_capacity(length),
            skipped: Vec::new(),
            parent,
        })
    }
}

impl SerializeStruct for StructSchemaBuilder<'_> {
    type Ok = SchemaBuilder;
    type Error = SerError;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.reserved_field_presence = self.parent.write_field_presence(
            self.reserved_field_presence,
            FieldIndex::try_from(self.field_names.len())?,
        )?;
        self.field_names.push(self.parent.intern_field_name(key)?);
        self.field_types
            .push(T::serialize(value, &mut *self.parent)?);
        Ok(())
    }

    #[inline]
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.skipped.push(self.field_names.len().try_into()?);
        self.field_names.push(self.parent.intern_field_name(key)?);
        self.field_types.push(SchemaBuilder::default());
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        let length = u32::try_from(self.field_names.len()).map_err(|_| SerError::TooManyValues)?;
        let field_names = Some(
            self.parent
                .fill_reserved_field_name_list(self.reserved_field_name_list, self.field_names)?,
        );
        Ok(SchemaBuilder::Record {
            name: Some(self.name),
            field_names,
            field_types: self.field_types,
            skippable: self.skipped,
            length,
        })
    }
}

impl SerializeStructVariant for StructSchemaBuilder<'_> {
    type Ok = SchemaBuilder;
    type Error = SerError;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        <Self as SerializeStruct>::serialize_field(self, key, value)
    }

    #[inline]
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        <Self as SerializeStruct>::skip_field(self, key)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as SerializeStruct>::end(self)
    }
}
