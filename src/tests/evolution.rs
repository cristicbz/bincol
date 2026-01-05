use super::helpers::{check_evolution_fails, check_evolution_ok};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, IgnoredAny},
};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug)]
pub struct UnitStruct;

impl From<String> for UnitStruct {
    fn from(_field: String) -> Self {
        Self
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewType<T = String>(T);

impl<T> From<String> for NewType<T>
where
    T: From<String>,
{
    fn from(field: String) -> Self {
        Self(field.into())
    }
}

impl<T> AsRef<str> for NewType<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TupleStruct<T = String, U = ()>(T, U);

impl<T, U> From<String> for TupleStruct<T, U>
where
    T: From<String>,
    U: Default,
{
    fn from(field: String) -> Self {
        Self(field.into(), U::default())
    }
}

impl<T, U> AsRef<str> for TupleStruct<T, U>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FieldStruct<T = String> {
    field: T,
}

impl<T> From<String> for FieldStruct<T>
where
    T: From<String>,
{
    fn from(field: String) -> Self {
        Self {
            field: field.into(),
        }
    }
}

impl<T> AsRef<str> for FieldStruct<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.field.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Enum<T = String, U = ()> {
    Unit,
    NewType(T),
    Tuple(T, U),
    Struct { field: T },
}

impl<T> AsRef<str> for Enum<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        match self {
            Self::NewType(field) | Self::Tuple(field, _) | Self::Struct { field } => field.as_ref(),
            Self::Unit => unreachable!(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct UnitVariant<T = String>(Enum<T>);

impl<T> From<String> for UnitVariant<T>
where
    T: From<String>,
{
    fn from(_field: String) -> Self {
        Self(Enum::Unit)
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct NewTypeVariant<T = String>(Enum<T>);

impl<T> From<String> for NewTypeVariant<T>
where
    T: From<String>,
{
    fn from(field: String) -> Self {
        Self(Enum::NewType(field.into()))
    }
}

impl<T> AsRef<str> for NewTypeVariant<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct TupleVariant<T = String, U = ()>(Enum<T, U>);

impl<T, U> From<String> for TupleVariant<T, U>
where
    T: From<String>,
    U: Default,
{
    fn from(field: String) -> Self {
        Self(Enum::Tuple(field.into(), U::default()))
    }
}

impl<T> AsRef<str> for TupleVariant<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct StructVariant<T = String>(Enum<T>);

impl<T> From<String> for StructVariant<T>
where
    T: From<String>,
{
    fn from(field: String) -> Self {
        Self(Enum::Struct {
            field: field.into(),
        })
    }
}

impl<T> AsRef<str> for StructVariant<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
struct OptionSome<T = String>(Option<T>);

impl<T> From<String> for OptionSome<T>
where
    T: From<String>,
{
    fn from(field: String) -> Self {
        Self(Some(field.into()))
    }
}

impl<T> AsRef<str> for OptionSome<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref().unwrap().as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
struct OptionNone<T = String>(Option<T>);

impl<T> From<T> for OptionNone<T> {
    fn from(_field: T) -> Self {
        Self(None)
    }
}

macro_rules! tests {
    ($(fn $name:ident<$t:ty, $u:ty>($check:ident);)+) => {
        $(
            #[test]
            fn $name() {
                $check::<$t, $u>();
            }
        )+
    };
}

tests! {
    fn test_string_to_ignored_any_ok<String, IgnoredAny>(check_ok_str_one_way);
    fn test_string_and_newtype_ok<String, NewType>(check_equal_str_two_way);
    fn test_string_and_tuple_struct_fail<String, TupleStruct>(check_fails_str_two_way);
    fn test_string_and_field_struct_fail<String, FieldStruct>(check_fails_str_two_way);
    fn test_string_and_newtype_variant_ok<String, NewTypeVariant>(check_equal_str_two_way);
    fn test_string_and_tuple_variant_fail<String, TupleVariant>(check_fails_str_two_way);
    fn test_string_and_struct_variant_fail<String, StructVariant>(check_fails_str_two_way);
    fn test_string_and_option_some_ok<String, OptionSome>(check_equal_str_two_way);
}

pub(crate) fn check_equal_str_two_way<
    T: DeserializeOwned + Serialize + Debug + AsRef<str> + From<String>,
    U: DeserializeOwned + Serialize + Debug + AsRef<str> + From<String>,
>() {
    check_equal_str_one_way::<T, U>();
    check_equal_str_one_way::<U, T>();
}

pub(crate) fn check_equal_str_one_way<
    T: Serialize + Debug + AsRef<str> + From<String>,
    U: DeserializeOwned + Debug + AsRef<str>,
>() {
    check_equal_str_one_way_leaf::<T, U>();
    check_equal_str_one_way_leaf::<NewType<T>, NewType<U>>();
    check_equal_str_one_way_leaf::<TupleStruct<T>, TupleStruct<U>>();
    check_equal_str_one_way_leaf::<FieldStruct<T>, FieldStruct<U>>();
    check_equal_str_one_way_leaf::<NewTypeVariant<T>, NewTypeVariant<U>>();
    check_equal_str_one_way_leaf::<TupleVariant<T>, TupleVariant<U>>();
    check_equal_str_one_way_leaf::<StructVariant<T>, StructVariant<U>>();
    check_equal_str_one_way_leaf::<OptionSome<T>, OptionSome<U>>();
}

pub(crate) fn check_equal_str_one_way_leaf<
    T: Serialize + Debug + AsRef<str> + From<String>,
    U: DeserializeOwned + Debug + AsRef<str>,
>() {
    check_evolution_ok::<_, U>(&T::from(String::new()), |old, new| {
        old.as_ref() == new.as_ref()
    });
    check_evolution_ok::<_, U>(&T::from("string".to_owned()), |old, new| {
        old.as_ref() == new.as_ref()
    });
}

pub(crate) fn check_ok_str_one_way<
    T: Serialize + Debug + From<String>,
    U: DeserializeOwned + Debug,
>() {
    check_ok_str_one_way_leaf::<T, U>();
    check_ok_str_one_way_leaf::<NewType<T>, NewType<U>>();
    check_ok_str_one_way_leaf::<TupleStruct<T>, TupleStruct<U>>();
    check_ok_str_one_way_leaf::<FieldStruct<T>, FieldStruct<U>>();
    check_ok_str_one_way_leaf::<NewTypeVariant<T>, NewTypeVariant<U>>();
    check_ok_str_one_way_leaf::<TupleVariant<T>, TupleVariant<U>>();
    check_ok_str_one_way_leaf::<StructVariant<T>, StructVariant<U>>();
    check_ok_str_one_way_leaf::<OptionSome<T>, OptionSome<U>>();
}

pub(crate) fn check_ok_str_one_way_leaf<
    T: Serialize + Debug + From<String>,
    U: DeserializeOwned + Debug,
>() {
    check_evolution_ok::<_, U>(&T::from(String::new()), |_, _| true);
    check_evolution_ok::<_, U>(&T::from("string".to_owned()), |_, _| true);
}

pub(crate) fn check_fails_str_two_way<
    T: DeserializeOwned + Serialize + Debug + From<String>,
    U: DeserializeOwned + Serialize + Debug + From<String>,
>() {
    check_fails_str_one_way::<T, U>();
    check_fails_str_one_way::<U, T>();
}

pub(crate) fn check_fails_str_one_way<T, U>()
where
    T: Serialize + Debug + From<String>,
    U: DeserializeOwned + Debug,
{
    check_fails_str_one_way_leaf::<T, U>();
    check_fails_str_one_way_leaf::<NewType<T>, NewType<U>>();
    check_fails_str_one_way_leaf::<TupleStruct<T>, TupleStruct<U>>();
    check_fails_str_one_way_leaf::<FieldStruct<T>, FieldStruct<U>>();
    check_fails_str_one_way_leaf::<NewTypeVariant<T>, NewTypeVariant<U>>();
    check_fails_str_one_way_leaf::<TupleVariant<T>, TupleVariant<U>>();
    check_fails_str_one_way_leaf::<StructVariant<T>, StructVariant<U>>();
    check_fails_str_one_way_leaf::<OptionSome<T>, OptionSome<U>>();
}

pub(crate) fn check_fails_str_one_way_leaf<
    T: Serialize + Debug + From<String>,
    U: DeserializeOwned + Debug,
>() {
    check_evolution_fails::<_, U>(&T::from(String::new()));
    check_evolution_fails::<_, U>(&T::from("string".to_owned()));
}
