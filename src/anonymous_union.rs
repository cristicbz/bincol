pub(crate) fn serialized_anonymous_variant<ErrorT>(i_variant: u32) -> Result<&'static str, ErrorT>
where
    ErrorT: serde::ser::Error,
{
    UNION_ENUM_VARIANT_NAMES
        .get(usize::try_from(i_variant).expect("usize should be at least 32 bits"))
        .copied()
        .ok_or_else(|| {
            ErrorT::custom(format!(
                "too many union variants {i_variant} >= {MAX_UNION_ENUM_VARIANTS}"
            ))
        })
}

pub(crate) fn deserialized_anonymous_variants<ErrorT>(
    num_variants: usize,
) -> Result<&'static [&'static str], ErrorT>
where
    ErrorT: serde::de::Error,
{
    UNION_ENUM_VARIANT_NAMES
        .split_at_checked(num_variants)
        .map(|(variants, _)| variants)
        .ok_or_else(|| {
            ErrorT::custom(format!(
                "too many union variants {num_variants} > {MAX_UNION_ENUM_VARIANTS}"
            ))
        })
}

pub(crate) const MAX_UNION_ENUM_VARIANTS: usize = UNION_ENUM_VARIANT_NAMES.len();
pub(crate) const UNION_ENUM_NAME: &str = "Union";
pub(crate) const UNION_ENUM_VARIANT_NAMES: &[&str; 256] = &{
    const HEX: [u8; 16] = *b"0123456789abcdef";

    // Creates the variant names `_00`, `_01`, ..., `_ff` as byte arrays.
    const AS_BYTES: [[u8; 3]; 256] = {
        let mut variants = [[0u8; 3]; 256];
        let mut high_nibble = 0;
        while high_nibble < 16 {
            let mut low_nibble = 0;
            while low_nibble < 16 {
                variants[high_nibble * 16 + low_nibble] = [b'_', HEX[high_nibble], HEX[low_nibble]];
                low_nibble += 1;
            }
            high_nibble += 1;
        }
        variants
    };

    // Convert the byte byte arrays to `str`-s.
    let mut strings = [""; 256];
    let mut i_variant = 0;
    while i_variant < 256 {
        strings[i_variant] = match str::from_utf8(&AS_BYTES[i_variant]) {
            Ok(string) => string,
            Err(_) => unreachable!(),
        };
        i_variant += 1;
    }
    strings
};
