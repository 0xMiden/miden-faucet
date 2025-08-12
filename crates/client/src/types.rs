//! A collection of new types and safety wrappers used throughout the faucet.

use miden_client::asset::FungibleAsset;
use serde::Serialize;

/// Describes the asset amounts allowed by the faucet.
#[derive(Clone, Serialize)]
pub struct AssetOptions(Vec<AssetAmount>);

impl std::fmt::Display for AssetOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;

        let mut options = self.0.iter();
        if let Some(first) = options.next() {
            write!(f, " {first}")?;
        }
        for rest in options {
            write!(f, ", {rest}")?;
        }

        write!(f, " ]")
    }
}

impl AssetOptions {
    /// Creates [`AssetOptions`] if all options are valid [`AssetAmount`]'s
    ///
    /// The error value contains the invalid option.
    pub fn new(options: Vec<u64>) -> Result<Self, AssetAmountError> {
        Ok(Self(options.into_iter().map(AssetAmount::new).collect::<Result<Vec<_>, _>>()?))
    }
}

/// Represents a valid asset amount for a [`FungibleAsset`].
///
/// Can only be created via [`AssetOptions`].
///
/// A [`FungibleAsset`] has a maximum representable amount
/// and this type guarantees that its value is within this range.
#[derive(Copy, Clone, Debug, Serialize)]
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

    pub fn inner(self) -> u64 {
        self.0
    }

    /// Creates an [`AssetAmount`] from a base unit amount.
    ///
    /// Returns an error if the amount is greater than the maximum allowed amount.
    pub fn new(base_units: u64) -> Result<Self, AssetAmountError> {
        if base_units > Self::MAX {
            return Err(AssetAmountError::AssetAmountTooBig(base_units));
        }

        Ok(Self(base_units))
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
