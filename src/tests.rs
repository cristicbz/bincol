use crate::{Schema, described::SelfDescribed};
use maplit::{btreemap, btreeset};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_bytes::ByteBuf;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
};

fn if_zero(value: &u32) -> bool {
    *value == 0
}

fn to_self_described_ron<T: Serialize>(value: T) -> ron::Result<String> {
    ron::ser::to_string_pretty(
        &SelfDescribed(value),
        ron::ser::PrettyConfig::default()
            .struct_names(true)
            .number_suffixes(true),
    )
}

fn from_self_described_ron<'de, T: Deserialize<'de>>(bytes: &'de str) -> ron::Result<T> {
    Ok(ron::from_str::<SelfDescribed<T>>(bytes).map(|pair| pair.0)?)
}

fn to_self_described_bitcode<T: Serialize>(value: T) -> Vec<u8> {
    bitcode::serialize(&SelfDescribed(value)).unwrap()
}

fn from_self_described_bitcode<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> T {
    bitcode::deserialize::<SelfDescribed<T>>(bytes).unwrap().0
}

fn bitcode_roundtrip<T: Serialize + DeserializeOwned>(value: &T) -> T {
    from_self_described_bitcode(&to_self_described_bitcode(value))
}

fn to_self_described_postcard<T: Serialize>(value: T) -> Vec<u8> {
    postcard::to_stdvec(&SelfDescribed(value)).unwrap()
}

fn from_self_described_postcard<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> T {
    postcard::from_bytes::<SelfDescribed<T>>(bytes).unwrap().0
}

fn postcard_roundtrip<T: Serialize + DeserializeOwned>(value: &T) -> T {
    from_self_described_postcard(&to_self_described_postcard(value))
}

fn check_roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(original: &T) {
    let schema = Schema::display_for_value(original)
        .map(|display| display.to_string())
        .unwrap_or_else(|error| format!("<trace error: {error}>"));
    let self_described_ron = to_self_described_ron(original);
    let ron_roundtripped = self_described_ron
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|string| from_self_described_ron::<T>(string));
    let self_described_ron_str =
        self_described_ron.unwrap_or_else(|error| format!("error: {error}"));
    let ron_roundtripped_str = ron_roundtripped
        .as_ref()
        .map(|value| format!("{value:#?}"))
        .unwrap_or_else(|error| format!("error: {error}"));

    assert_eq!(
        Ok(original),
        ron_roundtripped.as_ref(),
        "ORIGINAL: {original:#?}\n\nSCHEMA: {schema:#}\n\nRON: {self_described_ron_str}\n\nRON (roundtripped): {ron_roundtripped_str}"
    );
    assert_eq!(original, &bitcode_roundtrip(original));
    assert_eq!(original, &postcard_roundtrip(original));
}

#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct UnitStruct;
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct NewtypeStruct(u32);
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct NewtypeUnitStruct(());
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct NewtypeTupleStruct((u32, u64));
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TupleStruct(u32, u64);
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct EmptyTupleStruct();
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct EmptyFieldStruct {}
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FieldStructOne {
    x: u32,
}
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FieldStructTwo {
    x: u32,
    y: u32,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
enum AllVariantKinds {
    Unit,
    Newtype(u32),
    NewtypeUnit(()),
    NewtypeTuple((u32, u64)),
    NewtypeStructUnit(UnitStruct),
    NewTypeStructNewtype(NewtypeStruct),
    NewTypeStructNewtypeUnit(NewtypeUnitStruct),
    NewTypeStructNewtypeTuple(NewtypeTupleStruct),
    NewTypeStructTuple(TupleStruct),
    NewTypeStructEmptyTuple(EmptyTupleStruct),
    NewTypeStructEmptyField(EmptyFieldStruct),
    NewTypeStructFieldOne(FieldStructOne),
    NewTypeStructFieldTwo(FieldStructTwo),
    NewTypeOption(Option<Box<AllVariantKinds>>),
    Tuple(u32, u64),
    EmptyTuple(),
    EmptyStruct {},
    StructOne { x: u32 },
    StructTwo { x: u32, y: u32 },
}

macro_rules! test_roundtrip {
    ($(fn $name:ident($value:expr);)+) => {
        mod roundtrip {
            use super::*;
            $(
                #[test]
                fn $name() {
                    check_roundtrip(&$value);
                }
            )+
        }
    };
}

test_roundtrip! {
    fn test_u8(1u8);
    fn test_u16(1u16);
    fn test_u32(1u32);
    fn test_u64(1u64);
    fn test_u128(1u128);
    fn test_zero_u8(0u8);
    fn test_zero_u16(0u16);
    fn test_zero_u32(0u32);
    fn test_zero_u64(0u64);
    fn test_zero_u128(0u128);
    fn test_max_u8(u8::MAX);
    fn test_max_u16(u16::MAX);
    fn test_max_u32(u32::MAX);
    fn test_max_u64(u64::MAX);
    fn test_max_u128(u128::MAX);
    fn test_negative_i8(-1i8);
    fn test_negative_i16(-1i16);
    fn test_negative_i32(-1i32);
    fn test_negative_i64(-1i64);
    fn test_negative_i128(-1i128);
    fn test_positive_i8(1i8);
    fn test_positive_i16(1i16);
    fn test_positive_i32(1i32);
    fn test_positive_i64(1i64);
    fn test_positive_i128(1i128);
    fn test_max_i8(i8::MAX);
    fn test_max_i16(i16::MAX);
    fn test_max_i32(i32::MAX);
    fn test_max_i64(i64::MAX);
    fn test_max_i128(i128::MAX);
    fn test_min_i8(i8::MIN);
    fn test_min_i16(i16::MIN);
    fn test_min_i32(i32::MIN);
    fn test_min_i64(i64::MIN);
    fn test_min_i128(i128::MIN);
    fn test_empty_bytes(ByteBuf::new());
    fn test_nonempty_bytes(ByteBuf::from(vec![0u8, 1u8]));
    fn test_empty_string(String::new());
    fn test_nonempty_string("hello".to_owned());
    fn test_char('c');
    fn test_unit(());
    fn test_unit_struct(UnitStruct);
    fn test_unit_variant(AllVariantKinds::Unit);
    fn test_none(None::<u32>);

    fn test_some(Some(10u32));
    fn test_newtype_struct(NewtypeStruct(1));
    fn test_newtype_variant(AllVariantKinds::Newtype(1));

    fn test_empty_map(BTreeMap::<String, u32>::new());
    fn test_nonempty_map(btreemap! { 1u32 => -1i64 });
    fn test_empty_sequence(Vec::<u32>::new());
    fn test_nonempty_sequence(vec![0u32, 1u32]);

    fn test_tuple_one((1,));
    fn test_tuple_two((1, 2));
    fn test_empty_tuple_struct(EmptyTupleStruct());
    fn test_tuple_struct(TupleStruct(1, 2));
    fn test_empty_tuple_variant(AllVariantKinds::EmptyTuple());
    fn test_tuple_variant(AllVariantKinds::Tuple(1, 2));

    fn test_empty_field_struct(EmptyFieldStruct {});
    fn test_field_struct_one(FieldStructOne { x: 1 });
    fn test_field_struct_two(FieldStructTwo { x: 1, y: 2 });
    fn test_empty_struct_variant(AllVariantKinds::EmptyStruct {});
    fn test_struct_one_variant(AllVariantKinds::StructOne { x: 1 });
    fn test_struct_two_variant(AllVariantKinds::StructTwo { x: 1, y: 2 });
    fn test_sequence_none_some(vec![None, Some(0u32)]);
    fn test_sequence_none_some_some(vec![None, Some(None), Some(Some(None)), Some(Some(Some(0u32)))]);

    fn test_enum_newtype_unit(AllVariantKinds::NewtypeUnit(()));
    fn test_enum_newtype_tuple(AllVariantKinds::NewtypeTuple((2, 3)));
    fn test_enum_newtype_struct_unit(AllVariantKinds::NewtypeStructUnit(UnitStruct));
    fn test_enum_newtype_struct_newtype(AllVariantKinds::NewTypeStructNewtype(NewtypeStruct(4)));
    fn test_enum_newtype_struct_newtype_unit(AllVariantKinds::NewTypeStructNewtypeUnit(NewtypeUnitStruct(())));
    fn test_enum_newtype_struct_newtype_tuple(AllVariantKinds::NewTypeStructNewtypeTuple(NewtypeTupleStruct((5, 6))));
    fn test_enum_newtype_struct_tuple(AllVariantKinds::NewTypeStructTuple(TupleStruct(7, 8)));
    fn test_enum_newtype_struct_empty_tuple(AllVariantKinds::NewTypeStructEmptyTuple(EmptyTupleStruct()));
    fn test_enum_newtype_struct_empty_field(AllVariantKinds::NewTypeStructEmptyField(EmptyFieldStruct {}));
    fn test_enum_newtype_struct_field_one(AllVariantKinds::NewTypeStructFieldOne(FieldStructOne { x: 9 }));
    fn test_enum_newtype_struct_field_two(AllVariantKinds::NewTypeStructFieldTwo(FieldStructTwo { x: 10, y : 11 }));
    fn test_enum_newtype_option_none(AllVariantKinds::NewTypeOption(None));
    fn test_enum_newtype_option_unit(AllVariantKinds::NewTypeOption(Some(Box::new(AllVariantKinds::Unit))));
    fn test_enum_newtype_option_newtype(AllVariantKinds::NewTypeOption(Some(Box::new(AllVariantKinds::Newtype(12)))));

    fn test_enum_some_newtype_option_none(Some(AllVariantKinds::NewTypeOption(None)));
    fn test_enum_some_newtype_option_unit(Some(AllVariantKinds::NewTypeOption(Some(Box::new(AllVariantKinds::Unit)))));
    fn test_enum_some_newtype_option_newtype(Some(AllVariantKinds::NewTypeOption(Some(Box::new(AllVariantKinds::Newtype(12))))));
}

#[test]
fn test_always_skipped() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct WithSkipped {
        never_skipped: u32,

        #[serde(default, skip_serializing_if = "if_zero")]
        always_skipped: u32,
    }

    let original = vec![
        WithSkipped {
            never_skipped: 1,
            always_skipped: 0,
        },
        WithSkipped {
            never_skipped: 2,
            always_skipped: 0,
        },
    ];
    check_roundtrip(&original);
}

#[test]
fn test_stress_skipped_fields() {
    macro_rules! definitions {
        (
            struct $struct_name:ident { $($field:ident,)+ }
            let $value_name:ident = vec![@];
        ) => {
            #[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
            struct $struct_name {
                $(
                    #[serde(default, skip_serializing_if = "if_zero")]
                    $field: u32,
                )+
            }

            let $value_name = vec![
                $(
                    $struct_name { $field: 1, ..Default::default() },
                )+
                $struct_name::default(),
                $struct_name { $($field: 10),+ },
            ];
        };
    }

    definitions! {
        struct Struct {
            f00, f01, f02, f03, f04, f05, f06, f07, f08, f09,
            f10, f11, f12, f13, f14, f15, f16, f17, f18, f19,
            f20, f21, f22, f23, f24, f25, f26, f27, f28, f29,
            f30, f31, f32, f33, f34, f35, f36, f37, f38, f39,
            f40, f41, f42, f43, f44, f45, f46, f47, f48, f49,
            f50, f51, f52, f53, f54, f55, f56, f57, f58, f59,
            f60, f61, f62, f63,
        }
        let original = vec![@];
    }

    for i_end in 0..original.len() {
        check_roundtrip(&original[0..i_end].to_owned());
    }
}

#[test]
fn test_stress_enum() {
    macro_rules! definitions {
        (
            enum $enum_name:ident { $($variant:ident,)+ }
            let $value_name:ident = vec![@];
        ) => {
            #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
            enum $enum_name {
                $(
                    $variant(u32),
                )+
            }

            let $value_name = vec![
                $(
                    Some($enum_name::$variant(1)),
                )+
                None,
            ];
        };
    }

    definitions! {
        enum Enum {
            V000, V001, V002, V003, V004, V005, V006, V007, V008, V009,
            V010, V011, V012, V013, V014, V015, V016, V017, V018, V019,
            V020, V021, V022, V023, V024, V025, V026, V027, V028, V029,
            V030, V031, V032, V033, V034, V035, V036, V037, V038, V039,
            V040, V041, V042, V043, V044, V045, V046, V047, V048, V049,
            V050, V051, V052, V053, V054, V055, V056, V057, V058, V059,
            V060, V061, V062, V063, V064, V065, V066, V067, V068, V069,
            V070, V071, V072, V073, V074, V075, V076, V077, V078, V079,
            V080, V081, V082, V083, V084, V085, V086, V087, V088, V089,
            V090, V091, V092, V093, V094, V095, V096, V097, V098, V099,

            V100, V101, V102, V103, V104, V105, V106, V107, V108, V109,
            V110, V111, V112, V113, V114, V115, V116, V117, V118, V119,
            V120, V121, V122, V123, V124, V125, V126, V127, V128, V129,
            V130, V131, V132, V133, V134, V135, V136, V137, V138, V139,
            V140, V141, V142, V143, V144, V145, V146, V147, V148, V149,
            V150, V151, V152, V153, V154, V155, V156, V157, V158, V159,
            V160, V161, V162, V163, V164, V165, V166, V167, V168, V169,
            V170, V171, V172, V173, V174, V175, V176, V177, V178, V179,
            V180, V181, V182, V183, V184, V185, V186, V187, V188, V189,
            V190, V191, V192, V193, V194, V195, V196, V197, V198, V199,

            V200, V201, V202, V203, V204, V205, V206, V207, V208, V209,
            V210, V211, V212, V213, V214, V215, V216, V217, V218, V219,
            V220, V221, V222, V223, V224, V225, V226, V227, V228, V229,
            V230, V231, V232, V233, V234, V235, V236, V237, V238, V239,
            V240, V241, V242, V243, V244, V245, V246, V247, V248, V249,
            V250, V251, V252, V253, V254, V255, V256, V257, V258, V259,
            V260, V261, V262, V263, V264, V265, V266, V267, V268, V269,
            V270, V271, V272, V273, V274, V275, V276, V277, V278, V279,
            V280, V281, V282, V283, V284, V285, V286, V287, V288, V289,
            V290, V291, V292, V293, V294, V295, V296, V297, V298, V299,
        }
        let original = vec![@];
    }

    for i_end in 0..original.len() {
        check_roundtrip(&original[0..i_end].to_owned());
    }
}

#[derive(Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
struct Complex {
    map: BTreeMap<Option<Key>, Config>,
    sources: BTreeSet<Source>,
}

#[derive(Default, PartialOrd, Ord, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum Key {
    #[default]
    Default,

    // Variant is never used.
    JustTag(String),

    IdTag {
        id: u32,
        tag: String,
    },
}

#[derive(Default, PartialOrd, Ord, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum Source {
    #[default]
    User,

    // Skipping tuple fields doesn't emit `skip_field`, so these should be treated as different
    // variants.
    Host(
        String,
        #[serde(default = "http_default", skip_serializing_if = "is_http")] u16,
    ),
}

#[derive(Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
struct Settings {
    source: Option<Source>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    flags: Option<u64>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
}

#[derive(Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
struct Config {
    encoded: bool,

    // This field will always be skipped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    legacy_flags: Vec<u64>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    settings: Vec<Settings>,
}

fn is_http(port: &u16) -> bool {
    *port == 80
}

fn http_default() -> u16 {
    80
}

#[test]
fn test_complex_default() {
    check_roundtrip(&Complex::default());
}

#[test]
fn test_complex_full() {
    check_roundtrip(&Complex {
        map: btreemap! {
            None => Config::default(),
            Some(Key::IdTag { tag:"global".to_owned(), id: 1 }) => Config {
                encoded: true,
                settings: vec![
                    Settings {
                        source: Some(Source::User),
                        flags: None,
                        name: "cristi".to_owned(),
                        description: String::new(),
                    },
                    Settings {
                        source: None,
                        flags: Some(1234),
                        name: "bob".to_owned(),
                        description: "this is a description".to_owned()
                    },
                ],
                legacy_flags: Vec::new(),
            },
            Some(Key::IdTag { tag: "local".to_owned(), id: 2 }) => Config {
                encoded: true,
                settings: Vec::new(),
                legacy_flags: Vec::new(),
            },
            Some(Key::Default) => Config {
                encoded: false,
                settings: vec![
                    Settings {
                        source: Some(Source::User),
                        flags: Some(111),
                        name: "marty".to_owned(),
                        description: "another description".to_owned()
                    },
                ],
                legacy_flags: Vec::new(),
            },
        },
        sources: btreeset![
            Source::User,
            Source::Host("example.com".to_owned(), 443),
            Source::Host("google.com".to_owned(), 80),
        ],
    });
}
