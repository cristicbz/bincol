use super::helpers::{check_evolution_fails, check_evolution_ok};
use serde::{
    de::{DeserializeOwned, IgnoredAny},
    Deserialize, Serialize,
};
use serde_bytes::ByteBuf;
use std::{fmt::Debug, marker::PhantomData};

#[derive(Debug)]
struct UnitStruct<T = String> {
    phantom: PhantomData<T>,
}

impl<T> From<T> for UnitStruct<T> {
    fn from(_field: T) -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<T> Serialize for UnitStruct<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_unit_struct("UnitStruct")
    }
}

impl<'de, T> Deserialize<'de> for UnitStruct<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T>(PhantomData<T>);
        impl<'de, T> serde::de::Visitor<'de> for Visitor<T> {
            type Value = UnitStruct<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "UnitStruct")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(UnitStruct {
                    phantom: PhantomData,
                })
            }
        }
        deserializer.deserialize_unit_struct("UnitStruct", Visitor(PhantomData))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewType<T = String>(T);

impl<T> NewType<T> {
    fn new(field: T) -> Self {
        Self(field)
    }

    fn as_inner(&self) -> &T {
        &self.as_inner()
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

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
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
struct Optional<T = String>(Option<T>);

impl<T> From<String> for Optional<T>
where
    T: From<String>,
{
    fn from(field: String) -> Self {
        Self(Some(field.into()))
    }
}

impl<T> AsRef<str> for Optional<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref().unwrap().as_ref()
    }
}

#[derive(Serialize, Debug)]
#[serde(transparent)]
struct OptionNone<T = String>(Option<T>);

impl<T> From<T> for OptionNone<T> {
    fn from(_field: T) -> Self {
        Self(None)
    }
}

#[derive(Serialize, Debug)]
#[serde(transparent)]
struct Unit<T = String> {
    inner: (),
    phantom: PhantomData<T>,
}

impl<T> Unit<T> {
    fn new(_field: T) -> Self {
        Self {
            inner: (),
            phantom: PhantomData,
        }
    }
}

macro_rules! impl_primitive_conversions {
    ($(impl < T $(, $typevar:ident)? > $($trait:ident),+ for $($type:ident<T>),+;)+) => {
        $(
            impl_primitive_conversions! { @one_invocation; ($($type,)+); ($($trait,)+); ($($typevar)?); }
        )+
    };
    (@one_invocation; ($($type:ident,)+); $traits:tt; $typevars:tt; ) => {
        $(
            impl_primitive_conversions! { @one_type; $type; $traits; $typevars; }
        )+
    };
    (@one_type; $type:ident; ($($trait:ident,)+); $typevars:tt;) => {
        $(
            impl_primitive_conversions! {
                @one_primitive;
                $type;
                $trait;
                (
                    i8, i16, i32, i64, i128,
                    u8, u16, u32, u64, u128,
                    f32, f64, char, bool,
                    String, ByteBuf,
                );
                $typevars;
            }
        )+
    };
    (@one_primitive; $type:ident; $trait:ident; ($($primitive:ty,)+); $typevars:tt;) => {
        $(
            impl_primitive_conversions! {
                @one_impl;
                $type;
                $trait;
                $primitive;
                $typevars;
            }
        )+
    };
    (@one_impl; $type:ident; AsRef; $primitive:ty; ($($typevar:ident)?);) => {
        impl<T$(, $typevar)?> AsRef<$primitive> for $type<T>
        where
            T: AsRef<$primitive>,
        {
            fn as_ref(&self) -> &$primitive {
                self.as_inner().as_ref()
            }
        }
    };
    (@one_impl; $type:ident; From; $primitive:ty; ($($typevar:ident)?);) => {
        impl<T$(, $typevar)?> From<$primitive> for $type<T>
        where
            T: From<$primitive>,
            $($typevar: Default,)?
        {
            fn from(field: $primitive) -> Self {
                Self::new(T::from(field))
            }
        }
    };
    (@primitives) => {
    };
}

impl_primitive_conversions! {
    impl<T> From for Unit<T>;
    impl<T> AsRef, From for NewType<T>;
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

//tests! {
//    fn test_string_to_ignored_any_ok<String, IgnoredAny>(check_ok_str_one_way);
//    fn test_string_and_newtype_equal<String, NewType>(check_equals_str_two_way);
//    fn test_string_and_tuple_struct_fail<String, TupleStruct>(check_fails_str_two_way);
//    fn test_string_and_field_struct_fail<String, FieldStruct>(check_fails_str_two_way);
//    fn test_string_and_option_some_equal<String, Optional>(check_equals_str_two_way);
//
//    fn test_newtype_variant_to_string_fail<NewTypeVariant, String>(check_fails_str_one_way);
//    fn test_tuple_variant_to_string_fail<TupleVariant, String>(check_fails_str_one_way);
//    fn test_struct_variant_to_string_fail<StructVariant, String>(check_fails_str_one_way);
//    fn test_option_none_to_string_fails<OptionNone, String>(check_fails_str_one_way);
//    fn test_unit_to_string_fails<(), String>(check_fails_str_one_way);
//    fn test_unit_struct_to_string_fails<UnitStruct, String>(check_fails_str_one_way);
//    fn test_unit_variant_to_string_fails<UnitVariant, String>(check_fails_str_one_way);
//
//}

//pub(crate) fn check_equals_str_two_way<
//    T: DeserializeOwned + Serialize + Debug + AsRef<str> + From<String>,
//    U: DeserializeOwned + Serialize + Debug + AsRef<str> + From<String>,
//>() {
//    check_equals_str_one_way::<T, U>();
//    check_equals_str_one_way::<U, T>();
//}
//
//pub(crate) fn check_equals_str_one_way<
//    T: Serialize + Debug + AsRef<str> + From<String>,
//    U: DeserializeOwned + Debug + AsRef<str>,
//>() {
//    check_equals_str_one_way_leaf::<T, U>();
//    check_equals_str_one_way_leaf::<NewType<T>, NewType<U>>();
//    check_equals_str_one_way_leaf::<TupleStruct<T>, TupleStruct<U>>();
//    check_equals_str_one_way_leaf::<FieldStruct<T>, FieldStruct<U>>();
//    check_equals_str_one_way_leaf::<NewTypeVariant<T>, Enum<U>>();
//    check_equals_str_one_way_leaf::<TupleVariant<T>, Enum<U>>();
//    check_equals_str_one_way_leaf::<StructVariant<T>, Enum<U>>();
//    check_equals_str_one_way_leaf::<Optional<T>, Optional<U>>();
//}
//
//pub(crate) fn check_equals_str_one_way_leaf<
//    T: Serialize + Debug + AsRef<str> + From<String>,
//    U: DeserializeOwned + Debug + AsRef<str>,
//>() {
//    check_evolution_ok::<_, U>(&T::from(String::new()), |old, new| {
//        old.as_ref() == new.as_ref()
//    });
//    check_evolution_ok::<_, U>(&T::from("string".to_owned()), |old, new| {
//        old.as_ref() == new.as_ref()
//    });
//}
//
//pub(crate) fn check_ok_str_one_way<
//    T: Serialize + Debug + From<String>,
//    U: DeserializeOwned + Debug,
//>() {
//    check_ok_str_one_way_leaf::<T, U>();
//    check_ok_str_one_way_leaf::<NewType<T>, NewType<U>>();
//    check_ok_str_one_way_leaf::<TupleStruct<T>, TupleStruct<U>>();
//    check_ok_str_one_way_leaf::<FieldStruct<T>, FieldStruct<U>>();
//    check_ok_str_one_way_leaf::<NewTypeVariant<T>, Enum<U>>();
//    check_ok_str_one_way_leaf::<TupleVariant<T>, Enum<U>>();
//    check_ok_str_one_way_leaf::<StructVariant<T>, Enum<U>>();
//    check_ok_str_one_way_leaf::<Optional<T>, Optional<U>>();
//}
//
//pub(crate) fn check_ok_str_one_way_leaf<
//    T: Serialize + Debug + From<String>,
//    U: DeserializeOwned + Debug,
//>() {
//    check_evolution_ok::<_, U>(&T::from(String::new()), |_, _| true);
//    check_evolution_ok::<_, U>(&T::from("string".to_owned()), |_, _| true);
//}
//
//pub(crate) fn check_fails_str_two_way<
//    T: DeserializeOwned + Serialize + Debug + From<String>,
//    U: DeserializeOwned + Serialize + Debug + From<String>,
//>() {
//    check_fails_str_one_way::<T, U>();
//    check_fails_str_one_way::<U, T>();
//}
//
//pub(crate) fn check_fails_str_one_way<T, U>()
//where
//    T: Serialize + Debug + From<String>,
//    U: DeserializeOwned + Debug,
//{
//    check_fails_str_one_way_leaf::<T, U>();
//    check_fails_str_one_way_leaf::<NewType<T>, NewType<U>>();
//    check_fails_str_one_way_leaf::<TupleStruct<T>, TupleStruct<U>>();
//    check_fails_str_one_way_leaf::<FieldStruct<T>, FieldStruct<U>>();
//    check_fails_str_one_way_leaf::<NewTypeVariant<T>, Enum<U>>();
//    check_fails_str_one_way_leaf::<TupleVariant<T>, Enum<U>>();
//    check_fails_str_one_way_leaf::<StructVariant<T>, Enum<U>>();
//    check_fails_str_one_way_leaf::<Optional<T>, Optional<U>>();
//}
//
//pub(crate) fn check_fails_str_one_way_leaf<
//    T: Serialize + Debug + From<String>,
//    U: DeserializeOwned + Debug,
//>() {
//    check_evolution_fails::<_, U>(&T::from(String::new()));
//    check_evolution_fails::<_, U>(&T::from("string".to_owned()));
//}
