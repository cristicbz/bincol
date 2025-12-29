use std::{fmt::Write, sync::LazyLock};

pub(crate) fn serialized_anonymous_variant<ErrorT>(i_variant: u32) -> Result<&'static str, ErrorT>
where
    ErrorT: serde::ser::Error,
{
    UNION_VARIANT_NAMES
        .get(usize::try_from(i_variant).expect("usize should be at least 32 bits"))
        .copied()
        .ok_or_else(|| {
            ErrorT::custom(format!(
                "too many union variants {i_variant} >= {NUM_ANONYMOUS_NAMES}"
            ))
        })
}

pub(crate) fn deserialized_anonymous_variants<ErrorT>(
    num_variants: usize,
) -> Result<&'static [&'static str], ErrorT>
where
    ErrorT: serde::de::Error,
{
    UNION_VARIANT_NAMES
        .split_at_checked(num_variants)
        .map(|(variants, _)| variants)
        .ok_or_else(|| {
            ErrorT::custom(format!(
                "too many union variants {num_variants} > {NUM_ANONYMOUS_NAMES}"
            ))
        })
}

pub(crate) const NUM_ANONYMOUS_NAMES: usize = 4096;
pub(crate) static UNION_ENUM_NAME: &str = "Union";
static UNION_VARIANT_NAMES: LazyLock<&[&str]> = LazyLock::new(|| {
    let mut buffer = String::with_capacity(4 * NUM_ANONYMOUS_NAMES);
    for i_anonymous in 0..NUM_ANONYMOUS_NAMES {
        write!(&mut buffer, "_{i_anonymous:03X}").expect("infallible write");
    }
    let mut names = Vec::with_capacity(NUM_ANONYMOUS_NAMES);
    let mut buffer: &'static str = buffer.leak();
    while let Some((name, new_buffer)) = buffer.split_at_checked(4) {
        names.push(name);
        buffer = new_buffer;
    }
    names.leak()
});
