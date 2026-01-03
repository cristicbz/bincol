//! TODO: crate level docs
//!
//! Known limitations:
//! * At most 64 "skippable" fields. This can be lifted by using a `BitVec` instead of as single
//!   u64 to keep track of them. This is a backwards compatible change that can be made in the
//!   future.
#![deny(missing_docs)]

pub(crate) mod anonymous_union;
pub(crate) mod builder;
pub(crate) mod de;
pub(crate) mod deferred;
pub(crate) mod described;
pub(crate) mod dump;
pub(crate) mod indices;
pub(crate) mod pool;
pub(crate) mod schema;
pub(crate) mod ser;
pub(crate) mod trace;

pub use builder::{SchemaBuilder, TraceError};
pub use described::{DescribedBy, SelfDescribed};
pub use schema::Schema;
pub use trace::Trace;

#[cfg(test)]
mod tests;

#[cfg(doctest)]
mod doctests;
