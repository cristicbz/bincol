use crate::{Schema, described::SelfDescribed};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::Debug;

pub(crate) fn to_self_described_ron<T: Serialize>(value: T) -> ron::Result<String> {
    ron::ser::to_string_pretty(
        &SelfDescribed(value),
        ron::ser::PrettyConfig::default()
            .struct_names(true)
            .number_suffixes(true),
    )
}

pub(crate) fn from_self_described_ron<'de, T: Deserialize<'de>>(bytes: &'de str) -> ron::Result<T> {
    Ok(ron::from_str::<SelfDescribed<T>>(bytes).map(|pair| pair.0)?)
}

pub(crate) fn to_self_described_bitcode<T: Serialize>(value: T) -> Vec<u8> {
    bitcode::serialize(&SelfDescribed(value)).unwrap()
}

pub(crate) fn from_self_described_bitcode<'de, T: Deserialize<'de>>(
    bytes: &'de [u8],
) -> Result<T, bitcode::Error> {
    bitcode::deserialize::<SelfDescribed<T>>(bytes).map(|wrapper| wrapper.0)
}

pub(crate) fn to_self_described_postcard<T: Serialize>(value: T) -> Vec<u8> {
    postcard::to_stdvec(&SelfDescribed(value)).unwrap()
}

pub(crate) fn from_self_described_postcard<'de, T: Deserialize<'de>>(
    bytes: &'de [u8],
) -> Result<T, postcard::Error> {
    postcard::from_bytes::<SelfDescribed<T>>(bytes).map(|wrapper| wrapper.0)
}

pub(crate) fn check_roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(original: &T) {
    check_evolution_ok::<T, T>(original, T::eq);
}

pub(crate) fn check_evolution_ok<T: Serialize + Debug, U: DeserializeOwned + Debug>(
    original: &T,
    mut condition: impl FnMut(&T, &U) -> bool,
) {
    let schema = Schema::display_for_value(original)
        .map(|display| display.to_string())
        .unwrap_or_else(|error| format!("<trace error: {error}>"));

    let self_described_ron = to_self_described_ron(original);
    let ron_roundtripped = self_described_ron
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|string| from_self_described_ron::<U>(string));
    let self_described_ron_str =
        self_described_ron.unwrap_or_else(|error| format!("error: {error}"));
    let ron_roundtripped_str = ron_roundtripped
        .as_ref()
        .map(|value| format!("{value:#?}"))
        .unwrap_or_else(|error| format!("error: {error}"));
    assert!(
        matches!(ron_roundtripped.as_ref(), Ok(roundtripped) if condition(original, roundtripped)),
        "ORIGINAL: {original:#?}\n\nSCHEMA: {schema:#}\n\nRON: {self_described_ron_str}\n\nRON (roundtripped): {ron_roundtripped_str}"
    );

    let bitcode_roundtripped =
        from_self_described_bitcode::<U>(&to_self_described_bitcode(original));
    assert!(
        matches!(bitcode_roundtripped.as_ref(), Ok(roundtripped) if condition(original, roundtripped)),
        "ORIGINAL: {original:#?}\n\nBITCODE: {bitcode_roundtripped:#?}\n\nSCHEMA: {schema:#}\n\nRON: {self_described_ron_str}\n\nRON (roundtripped): {ron_roundtripped_str}"
    );

    let postcard_roundtripped =
        from_self_described_postcard::<U>(&to_self_described_postcard(original));
    assert!(
        matches!(postcard_roundtripped.as_ref(), Ok(roundtripped) if condition(original, roundtripped)),
        "ORIGINAL: {original:#?}\n\nPOSTCARD: {postcard_roundtripped:#?}\n\nSCHEMA: {schema:#}\n\nRON: {self_described_ron_str}\n\nRON (roundtripped): {ron_roundtripped_str}"
    );
}

pub(crate) fn check_evolution_fails<T: Serialize + Debug, U: DeserializeOwned + Debug>(
    original: &T,
) {
    let schema = Schema::display_for_value(original)
        .map(|display| display.to_string())
        .unwrap_or_else(|error| format!("<trace error: {error}>"));

    let self_described_ron = to_self_described_ron(original);
    let ron_roundtripped = self_described_ron
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|string| from_self_described_ron::<U>(string));
    let self_described_ron_str =
        self_described_ron.unwrap_or_else(|error| format!("error: {error}"));
    let ron_roundtripped_str = ron_roundtripped
        .as_ref()
        .map(|value| format!("{value:#?}"))
        .unwrap_or_else(|error| format!("error: {error}"));
    assert!(
        ron_roundtripped.is_err(),
        "ORIGINAL: {original:#?}\n\nSCHEMA: {schema:#}\n\nRON: {self_described_ron_str}\n\nRON (roundtripped): {ron_roundtripped_str}"
    );

    let bitcode_roundtripped =
        from_self_described_bitcode::<U>(&to_self_described_bitcode(original));
    assert!(
        bitcode_roundtripped.as_ref().is_err(),
        "ORIGINAL: {original:#?}\n\nBITCODE: {bitcode_roundtripped:#?}\n\nSCHEMA: {schema:#}\n\nRON: {self_described_ron_str}\n\nRON (roundtripped): {ron_roundtripped_str}"
    );

    let postcard_roundtripped =
        from_self_described_postcard::<U>(&to_self_described_postcard(original));
    assert!(
        postcard_roundtripped.as_ref().is_err(),
        "ORIGINAL: {original:#?}\n\nPOSTCARD: {postcard_roundtripped:#?}\n\nSCHEMA: {schema:#}\n\nRON: {self_described_ron_str}\n\nRON (roundtripped): {ron_roundtripped_str}"
    );
}
