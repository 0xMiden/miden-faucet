//! A collection of new types and safety wrappers used throughout the faucet.

use miden_client::asset::FungibleAsset;

/// Describes the asset amounts allowed by the faucet.
#[derive(Clone)]
pub struct AssetOptions(pub Vec<u64>);

/// Represents a valid asset amount for a [`FungibleAsset`].
///
/// Can only be created via [`AssetOptions`].
///
/// A [`FungibleAsset`] has a maximum representable amount
/// and this type guarantees that its value is within this range.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct AssetAmount(u64);

impl std::fmt::Display for AssetAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AssetAmount {
    /// The absolute maximum asset amount allowed by the network.
    ///
    /// An [`AssetAmount`] is further restricted to the values allowed by
    /// [`AssetOptions`].
    pub const MAX: u64 = FungibleAsset::MAX_AMOUNT;

    /// Creates an [`AssetAmount`] from a base unit amount.
    ///
    /// Returns an error if the amount is greater than the maximum allowed amount.
    pub fn new(base_units: u64) -> Result<Self, AssetAmountError> {
        if base_units > Self::MAX {
            return Err(AssetAmountError::AssetAmountTooBig(base_units));
        }

        Ok(Self(base_units))
    }

    pub fn from_tokens(tokens: u64, decimals: u8) -> Result<Self, AssetAmountError> {
        Self::new(tokens * 10u64.pow(u32::from(decimals)))
    }

    /// Returns the asset amount in base units.
    pub fn base_units(&self) -> u64 {
        self.0
    }

    /// Returns the asset amount in tokens.
    ///
    /// Returns a string representation to avoid precision loss. This is only meant for display
    /// purposes.
    pub fn tokens(&self, decimals: u8) -> String {
        // This code was adapted from miden-client: https://github.com/0xMiden/miden-client/blob/88ccbe4/bin/miden-cli/src/faucet_details_map.rs#L131
        let units_str = self.base_units().to_string();
        let len = units_str.len();

        if decimals == 0 {
            return units_str;
        }

        if decimals as usize >= len {
            "0.".to_owned() + &"0".repeat(decimals as usize - len) + &units_str
        } else {
            // Insert the decimal point at the correct position
            let integer_part = &units_str[..len - decimals as usize];
            let fractional_part = &units_str[len - decimals as usize..];
            format!("{integer_part}.{fractional_part}")
        }
    }

    /// Adds another [`AssetAmount`] to the current one and returns the result if it is valid.
    pub fn add_amount(self, other: Self) -> Option<Self> {
        Self::new(self.0 + other.0).ok()
    }

    /// Subtracts another [`AssetAmount`] from the current one and returns the result if it is
    /// valid.
    pub fn sub_amount(self, other: Self) -> Option<Self> {
        Self::new(self.0 - other.0).ok()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AssetAmountError {
    #[error(
        "fungible asset amount {0} exceeds the max allowed amount of {max_amount}",
        max_amount = FungibleAsset::MAX_AMOUNT
      )]
    AssetAmountTooBig(u64),
}

/// Type of note to generate for a mint request.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NoteType {
    Private,
    Public,
}

impl From<NoteType> for miden_client::note::NoteType {
    fn from(value: NoteType) -> Self {
        match value {
            NoteType::Private => Self::Private,
            NoteType::Public => Self::Public,
        }
    }
}

impl std::fmt::Display for NoteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Private => f.write_str("private"),
            Self::Public => f.write_str("public"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_amount_converts_to_tokens() {
        #[allow(clippy::unreadable_literal)]
        let asset_amount = AssetAmount::new(123456789123456789).unwrap();
        assert_eq!(asset_amount.tokens(0), "123456789123456789");
        assert_eq!(asset_amount.tokens(1), "12345678912345678.9");
        assert_eq!(asset_amount.tokens(2), "1234567891234567.89");
        assert_eq!(asset_amount.tokens(3), "123456789123456.789");
        assert_eq!(asset_amount.tokens(4), "12345678912345.6789");
        assert_eq!(asset_amount.tokens(5), "1234567891234.56789");
        assert_eq!(asset_amount.tokens(18), "0.123456789123456789");
        assert_eq!(asset_amount.tokens(19), "0.0123456789123456789");
    }
}
