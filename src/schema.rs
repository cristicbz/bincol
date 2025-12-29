use serde::{de::DeserializeSeed, Deserialize, Serialize};
use std::{hash::Hash, marker::PhantomData};
use thiserror::Error;

use crate::{
    builder::Value,
    indices::{
        FieldIndex, FieldListIndex, NameIndex, NameListIndex, SchemaNodeIndex, SchemaNodeListIndex,
    },
    DescribedBy,
};

/// A saved schema that describes serialized data in a non-self-describing format.
///
/// Produced via a [`crate::SchemaBuilder`] which traces the various serialized types, see that
/// type's documentation for a complete example.
///
/// For simple use-cases where the [`Schema`] should be serialized together with the data, use
/// the [`crate::SelfDescribed`] wrapper, which obviates the need for an explicitly exposed
/// [`Schema`] object.
#[derive(Clone, Serialize, Deserialize)]
pub struct Schema {
    pub(crate) root_index: SchemaNodeIndex,
    pub(crate) nodes: Box<[SchemaNode]>,
    pub(crate) names: Box<[Box<str>]>,
    pub(crate) name_lists: Box<[Box<[NameIndex]>]>,
    pub(crate) node_lists: Box<[Box<[SchemaNodeIndex]>]>,
    pub(crate) field_lists: Box<[Box<[FieldIndex]>]>,
}

impl Schema {
    /// Returns a [`serde::de::DeserializeSeed`] for a value to be deserialized using this schema.
    ///
    /// If you don't need a shared schema, use the much simpler [`crate::SelfDescribed`] wrapper
    /// wrapper.
    pub fn describe_type<'schema, 'de, T>(&'schema self) -> DescribedBy<'schema, PhantomData<T>>
    where
        T: Deserialize<'de>,
    {
        DescribedBy(PhantomData, self)
    }

    /// Returns a [`serde::Serialize`]-able wrapper for a [`crate::Value`].
    ///
    /// You will need to provide the schema again at deserialization-time using
    /// [`Self::describe_type`] or [`Self::describe_seed`].
    pub fn describe_value<'schema>(&'schema self, value: Value) -> DescribedBy<'schema, Value> {
        DescribedBy(value, self)
    }

    /// Returns a [`serde::Serialize`]-able wrapper for a reference to a [`crate::Value`]..
    ///
    /// You will need to provide the schema again at deserialization-time using
    /// [`Self::describe_type`] or [`Self::describe_seed`].
    pub fn describe_value_ref<'schema, 'value>(
        &'schema self,
        value: &'value Value,
    ) -> DescribedBy<'schema, &'value Value> {
        DescribedBy(value, self)
    }

    /// Wraps a [`serde::de::DeserializeSeed`] to be deserialized using this schema.
    ///
    /// If you don't need your own seed, you can use [`Self::describe_type`] instead.
    ///
    /// Example
    /// -------
    /// ```rust
    /// use serde::de::{Deserialize, Deserializer, DeserializeSeed};
    /// use serde_describe::{SchemaBuilder, DescribedBy};
    ///
    /// /// Deserializes an integer by multiplying it by a given constant.
    /// pub struct Multiplier {
    ///     pub by: u32,
    /// }
    ///
    /// impl<'de> serde::de::DeserializeSeed<'de> for Multiplier {
    ///     type Value = u32;
    ///
    ///     fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    ///     where
    ///         D: Deserializer<'de>,
    ///     {
    ///         Ok(u32::deserialize(deserializer)? * self.by)
    ///     }
    /// }
    ///
    /// let mut builder = SchemaBuilder::new();
    /// let value = builder.trace_value(&10u32)?;
    /// let schema = builder.build()?;
    ///
    /// let serialized = postcard::to_stdvec(
    ///     &schema.describe_value(value)
    /// )?;
    /// let DescribedBy(deserialized, _) = schema
    ///     .describe_seed(Multiplier { by: 2 })
    ///     .deserialize(&mut postcard::Deserializer::from_bytes(&serialized))?;
    ///
    /// assert_eq!(deserialized, 20);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn describe_seed<'schema, 'de, SeedT>(
        &'schema self,
        seed: SeedT,
    ) -> DescribedBy<'schema, SeedT>
    where
        SeedT: DeserializeSeed<'de>,
    {
        DescribedBy(seed, self)
    }

    #[inline]
    pub(crate) fn name(&self, index: NameIndex) -> Result<&str, NoSuchNameError> {
        self.names
            .get(usize::from(index))
            .map(|string| &**string)
            .ok_or(NoSuchNameError(index))
    }

    #[inline]
    pub(crate) fn name_list(
        &self,
        index: NameListIndex,
    ) -> Result<&[NameIndex], NoSuchNameListError> {
        self.name_lists
            .get(usize::from(index))
            .map(|list| &**list)
            .ok_or(NoSuchNameListError(index))
    }

    #[inline]
    pub(crate) fn node(&self, index: SchemaNodeIndex) -> Result<SchemaNode, NoSuchSchemaError> {
        self.nodes
            .get(usize::from(index))
            .copied()
            .ok_or(NoSuchSchemaError(index))
    }

    #[inline]
    pub(crate) fn node_list(
        &self,
        index: SchemaNodeListIndex,
    ) -> Result<&[SchemaNodeIndex], NoSuchSchemaListError> {
        self.node_lists
            .get(usize::from(index))
            .map(|list| &**list)
            .ok_or(NoSuchSchemaListError(index))
    }

    #[inline]
    pub(crate) fn field_list(
        &self,
        index: FieldListIndex,
    ) -> Result<&[FieldIndex], NoSuchFieldListError> {
        self.field_lists
            .get(usize::from(index))
            .map(|list| &**list)
            .ok_or(NoSuchFieldListError(index))
    }
}

#[derive(Clone, Copy, Debug, Error)]
#[error("no such name with index {0:?}")]
pub(crate) struct NoSuchNameError(NameIndex);

#[derive(Clone, Copy, Debug, Error)]
#[error("no such name list with index {0:?}")]
pub(crate) struct NoSuchNameListError(NameListIndex);

#[derive(Clone, Copy, Debug, Error)]
#[error("no such node with index {0:?}")]
pub(crate) struct NoSuchSchemaError(SchemaNodeIndex);

#[derive(Clone, Copy, Debug, Error)]
#[error("no such node list with index {0:?}")]
pub(crate) struct NoSuchSchemaListError(SchemaNodeListIndex);

#[derive(Clone, Copy, Debug, Error)]
#[error("no such field list with index {0:?}")]
pub(crate) struct NoSuchFieldListError(FieldListIndex);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum SchemaNode {
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
    OptionSome(SchemaNodeIndex),

    Unit,
    UnitStruct(NameIndex),
    UnitVariant(NameIndex, NameIndex),

    NewtypeStruct(NameIndex, SchemaNodeIndex),
    NewtypeVariant(NameIndex, NameIndex, SchemaNodeIndex),

    Sequence(SchemaNodeIndex),
    Map(SchemaNodeIndex, SchemaNodeIndex),

    Tuple(u32, SchemaNodeListIndex),
    TupleStruct(NameIndex, u32, SchemaNodeListIndex),
    TupleVariant(NameIndex, NameIndex, u32, SchemaNodeListIndex),

    Struct(
        NameIndex,
        NameListIndex,
        FieldListIndex,
        SchemaNodeListIndex,
    ),
    StructVariant(
        NameIndex,
        NameIndex,
        NameListIndex,
        FieldListIndex,
        SchemaNodeListIndex,
    ),

    Union(SchemaNodeListIndex),
}
