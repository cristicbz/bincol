use itertools::Itertools;
use serde::{
    Deserialize, Serialize,
};
use std::{
    fmt::{Debug, Write},
    hash::Hash,
};
use thiserror::Error;

use crate::indices::{
    FieldIndex, FieldListIndex, NameIndex, NameListIndex, SchemaIndex, SchemaListIndex,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct RootSchema {
    pub(crate) schemas: Box<[Schema]>,
    pub(crate) names: Box<[Box<str>]>,
    pub(crate) name_lists: Box<[Box<[NameIndex]>]>,
    pub(crate) schema_lists: Box<[Box<[SchemaIndex]>]>,
    pub(crate) field_lists: Box<[Box<[FieldIndex]>]>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Schema {
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
    OptionSome(SchemaIndex),

    Unit,
    UnitStruct(NameIndex),
    UnitVariant(NameIndex, NameIndex),

    NewtypeStruct(NameIndex, SchemaIndex),
    NewtypeVariant(NameIndex, NameIndex, SchemaIndex),

    Sequence(SchemaIndex),
    Map(SchemaIndex, SchemaIndex),

    Tuple(u32, SchemaListIndex),
    TupleStruct(NameIndex, u32, SchemaListIndex),
    TupleVariant(NameIndex, NameIndex, u32, SchemaListIndex),

    Struct(NameIndex, NameListIndex, FieldListIndex, SchemaListIndex),
    StructVariant(
        NameIndex,
        NameIndex,
        NameListIndex,
        FieldListIndex,
        SchemaListIndex,
    ),

    Union(SchemaListIndex),
}

impl RootSchema {
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
    pub(crate) fn schema(&self, index: SchemaIndex) -> Result<Schema, NoSuchSchemaError> {
        self.schemas
            .get(usize::from(index))
            .copied()
            .ok_or(NoSuchSchemaError(index))
    }

    #[inline]
    pub(crate) fn schema_list(
        &self,
        index: SchemaListIndex,
    ) -> Result<&[SchemaIndex], NoSuchSchemaListError> {
        self.schema_lists
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

    pub(crate) fn dump(&self, indent: &mut String, index: SchemaIndex) -> Result<(), DumpError> {
        if indent.is_empty() {
            eprintln!("SCHEMA:")
        }
        indent.push_str("  ");
        let schema = self.schema(index)?;
        match schema {
            Schema::Bool
            | Schema::I8
            | Schema::I16
            | Schema::I32
            | Schema::I64
            | Schema::I128
            | Schema::U8
            | Schema::U16
            | Schema::U32
            | Schema::U64
            | Schema::U128
            | Schema::F32
            | Schema::F64
            | Schema::Char
            | Schema::String
            | Schema::Bytes
            | Schema::Unit => eprintln!("{indent}{schema:?},"),
            Schema::OptionNone => eprintln!("{indent}::Option::None"),

            Schema::UnitStruct(name) => eprintln!("{}{},", indent, self.name(name)?),
            Schema::UnitVariant(name, variant) => {
                eprintln!("{}{}::{},", indent, self.name(name)?, self.name(variant)?)
            }

            Schema::OptionSome(inner) => {
                eprintln!("{}::Option::Some(", indent);
                self.dump(indent, inner)?;
                eprintln!("{indent}),")
            }
            Schema::NewtypeStruct(name, inner) => {
                eprintln!("{}{}(", indent, self.name(name)?);
                self.dump(indent, inner)?;
                eprintln!("{indent}),")
            }
            Schema::NewtypeVariant(name, variant, inner) => {
                eprintln!("{}{}::{}(", indent, self.name(name)?, self.name(variant)?);
                self.dump(indent, inner)?;
                eprintln!("{indent}),")
            }
            Schema::Map(key, value) => {
                eprintln!("{indent}{{");
                self.dump(indent, key)?;
                self.dump(indent, value)?;
                eprintln!("{indent}}},")
            }
            Schema::Sequence(item) => {
                eprintln!("{indent}[");
                self.dump(indent, item)?;
                eprintln!("{indent}],")
            }

            Schema::Tuple(_, schema_list) => {
                eprintln!("{indent}(");
                for &schema in self.schema_list(schema_list)? {
                    self.dump(indent, schema)?;
                }
                eprintln!("{indent}),")
            }

            Schema::TupleStruct(name, _, schema_list) => {
                eprintln!("{}{}(", indent, self.name(name)?);
                for &schema in self.schema_list(schema_list)? {
                    self.dump(indent, schema)?;
                }
                eprintln!("{indent}),")
            }
            Schema::TupleVariant(name, variant, _, schema_list) => {
                eprintln!("{}{}::{}(", indent, self.name(name)?, self.name(variant)?);
                for &schema in self.schema_list(schema_list)? {
                    self.dump(indent, schema)?;
                }
                eprintln!("{indent}),")
            }

            Schema::Struct(name, name_list, skip_list, type_list) => {
                eprintln!("{}{} {{", indent, self.name(name)?);
                indent.push_str("  ");
                let mut skips = self.field_list(skip_list)?;
                let has_skips = !skips.is_empty();
                for (i_field, (&name, &schema)) in self
                    .name_list(name_list)?
                    .iter()
                    .zip(self.schema_list(type_list)?)
                    .enumerate()
                {
                    if has_skips {
                        let required = if let Some(&i_next_skip) = skips.first()
                            && usize::from(i_next_skip) == i_field
                        {
                            skips.split_off_first();
                            "?"
                        } else {
                            ""
                        };
                        eprintln!("{}{}{}:", indent, self.name(name)?, required);
                    } else {
                        eprintln!("{}{}:", indent, self.name(name)?);
                    }
                    self.dump(indent, schema)?;
                }
                indent.truncate(indent.len() - 2);
                eprintln!("{indent}}},")
            }
            Schema::StructVariant(name, variant, name_list, skip_list, type_list) => {
                eprintln!("{}{}::{} {{", indent, self.name(name)?, self.name(variant)?);
                indent.push_str("  ");
                let mut skips = self.field_list(skip_list)?;
                let has_skips = !skips.is_empty();
                for (i_field, (&name, &schema)) in self
                    .name_list(name_list)?
                    .iter()
                    .zip(self.schema_list(type_list)?)
                    .enumerate()
                {
                    if has_skips {
                        let required = if let Some(&i_next_skip) = skips.first()
                            && usize::from(i_next_skip) == i_field
                        {
                            skips.split_off_first();
                            "optional"
                        } else {
                            "required"
                        };
                        eprintln!("{}{} [{}]:", indent, self.name(name)?, required);
                    } else {
                        eprintln!("{}{}:", indent, self.name(name)?);
                    }
                    self.dump(indent, schema)?;
                }
                indent.truncate(indent.len() - 2);
                eprintln!("{indent}}},")
            }

            Schema::Union(type_list) => {
                eprintln!("{indent}<");
                indent.push_str("  ");
                for &schema in self.schema_list(type_list)? {
                    self.dump(indent, schema)?;
                }
                indent.truncate(indent.len() - 2);
                eprintln!("{indent}>,")
            }
        }
        indent.truncate(indent.len() - 2);
        if indent.is_empty() {
            eprintln!("\n\n")
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Error)]
#[error("no such name with index {0:?}")]
pub(crate) struct NoSuchNameError(NameIndex);

#[derive(Clone, Copy, Debug, Error)]
#[error("no such name list with index {0:?}")]
pub(crate) struct NoSuchNameListError(NameListIndex);

#[derive(Clone, Copy, Debug, Error)]
#[error("no such schema with index {0:?}")]
pub(crate) struct NoSuchSchemaError(SchemaIndex);

#[derive(Clone, Copy, Debug, Error)]
#[error("no such schema list with index {0:?}")]
pub(crate) struct NoSuchSchemaListError(SchemaListIndex);

#[derive(Clone, Copy, Debug, Error)]
#[error("no such field list with index {0:?}")]
pub(crate) struct NoSuchFieldListError(FieldListIndex);

#[derive(Clone, Copy, Debug, Error)]
pub(crate) enum DumpError {
    #[error("dump error: {0}")]
    NoSuchName(#[from] NoSuchNameError),

    #[error("dump error: {0}")]
    NoSuchNameList(#[from] NoSuchNameListError),

    #[error("dump error: {0}")]
    NoSuchSchema(#[from] NoSuchSchemaError),

    #[error("dump error: {0}")]
    NoSuchSchemaList(#[from] NoSuchSchemaListError),

    #[error("dump error: {0}")]
    NoSuchFieldList(#[from] NoSuchFieldListError),
}
