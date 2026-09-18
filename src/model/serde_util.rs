use serde::{Deserialize, Deserializer};

/// 5etools' `_copy`/`_mod` override convention sometimes sets a field to
/// `null` to mean "empty" rather than omitting it - `#[serde(default)]`
/// alone only covers a *missing* field, not a present-but-null one. Pair
/// this with `#[serde(default)]` on any field that can appear as `null` in
/// the corpus.
pub fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}
