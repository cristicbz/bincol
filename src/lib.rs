// #![deny(missing_docs)]

pub(crate) mod anonymous_union;
pub(crate) mod builder;
pub(crate) mod de;
pub(crate) mod deferred;
pub(crate) mod described;
pub(crate) mod errors;
pub(crate) mod indices;
pub(crate) mod pool;
pub(crate) mod schema;
pub(crate) mod ser;
pub(crate) mod trace;
pub(crate) mod value;

pub use described::{Described, DescribedElsewhere};
pub use schema::RootSchema;
pub use value::{Value, ValueData};
