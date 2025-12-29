use crate::{builder::RootSchemaBuilder, errors::SerError, indices::SchemaIndex, RootSchema};
use serde::Serialize;

#[derive(Default, Clone, Serialize)]
pub struct ValueData(pub(crate) Vec<u8>);

#[derive(Clone)]
pub struct Value {
    pub schema: RootSchema,
    pub root_index: SchemaIndex,
    pub data: ValueData,
}

pub fn to_value<SerializeT>(value: &SerializeT) -> Result<Value, SerError>
where
    SerializeT: ?Sized + Serialize,
{
    RootSchemaBuilder::new(value)?.into_value()
}
