use std::marker::PhantomData;

use crate::RootSchema;

#[derive(Copy, Clone)]
pub struct Described<T>(pub T);

pub struct DescribedElsewhere<'schema, T>(pub &'schema RootSchema, pub PhantomData<T>);

impl<'schema, T> Copy for DescribedElsewhere<'schema, T> {}
impl<'schema, T> Clone for DescribedElsewhere<'schema, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
