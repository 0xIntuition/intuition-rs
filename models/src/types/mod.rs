use std::{
    fmt::Display,
    ops::{Add, Sub, SubAssign},
    str::FromStr,
};

use crate::error::ModelError;
use alloy::primitives::U256;
use serde::{Deserialize, Serialize};
use sqlx::{
    encode::IsNull,
    postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef},
    types::BigDecimal,
    Encode, Postgres, Result, Type,
};

/// This is a wrapper around the `U256` type to be able to use it with
/// the `sqlx` library.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct U256Wrapper(pub U256);

impl TryInto<U256> for U256Wrapper {
    type Error = ModelError;

    fn try_into(self) -> Result<U256, Self::Error> {
        Ok(self.0)
    }
}

impl Default for U256Wrapper {
    fn default() -> Self {
        U256Wrapper(U256::ZERO)
    }
}

impl Add for U256Wrapper {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        U256Wrapper(self.0 + other.0)
    }
}

impl Sub for U256Wrapper {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        U256Wrapper(self.0 - other.0)
    }
}

impl SubAssign for U256Wrapper {
    fn sub_assign(&mut self, other: Self) {
        self.0 -= other.0;
    }
}

impl Display for U256Wrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<i64> for U256Wrapper {
    type Error = ModelError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(U256Wrapper(U256::from(value)))
    }
}

impl TryFrom<BigDecimal> for U256Wrapper {
    type Error = ModelError;

    fn try_from(value: BigDecimal) -> Result<Self, Self::Error> {
        U256::from_str(&value.to_string())
            .map(U256Wrapper)
            .map_err(|e| ModelError::ConversionError(e.to_string()))
    }
}

/// This is a method to convert the `U256` type to a `U256Wrapper` type.
impl From<U256> for U256Wrapper {
    fn from(value: U256) -> Self {
        U256Wrapper(value)
    }
}

impl U256Wrapper {
    /// This is a method to convert the `U256Wrapper` to a `BigDecimal`
    /// type. This is necessary because the `sqlx` library does not
    /// natively support the `U256` type.
    pub fn to_big_decimal(&self) -> std::result::Result<BigDecimal, ModelError> {
        BigDecimal::from_str(&self.0.to_string())
            .map_err(|e| ModelError::ConversionError(e.to_string()))
    }

    /// This is a method to add two `U256Wrapper` types
    pub fn add(&self, other: U256Wrapper) -> Result<U256Wrapper, ModelError> {
        Ok(U256Wrapper(self.0 + other.0))
    }
}

/// We are implementing the `FromStr` trait for the `U256Wrapper` type
/// to be able to convert a string to a `U256Wrapper` type.
impl FromStr for U256Wrapper {
    type Err = ModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        U256::from_str(value)
            .map(U256Wrapper)
            .map_err(|e| ModelError::ConversionError(e.to_string()))
    }
}

/// This is a method to return the type info for the `U256Wrapper` type.
/// This is necessary because the `sqlx` library needs to know the type
/// of the column to be able to convert it to the correct type.
impl Type<Postgres> for U256Wrapper {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("numeric")
    }
}

/// This is a method to encode the `U256Wrapper` type to a `PgArgumentBuffer`
/// type. This is necessary because the `sqlx` library needs to be able to
/// convert the type to the correct one.
impl Encode<'_, Postgres> for U256Wrapper {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        let s = self.0.to_string();
        <&str as Encode<Postgres>>::encode(&s, buf)
    }
}

/// This is a method to decode the `U256Wrapper` type from a `PgValueRef`
/// type. This is necessary because the `sqlx` library needs to be able to
/// convert the type to the correct one.
// Start of Selection
/// Implements the `Decode` trait for `U256Wrapper` to enable decoding from PostgreSQL.
/// This allows `sqlx` to correctly deserialize `numeric` types into `U256Wrapper`.
impl<'r> sqlx::Decode<'r, Postgres> for U256Wrapper {
    fn decode(
        value: PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        // First, decode the value as `BigDecimal`.
        let bd: BigDecimal = BigDecimal::decode(value)?;

        let u256 = U256::from_str(&bd.to_string()).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
        // Wrap the parsed `U256` in `U256Wrapper` and return.
        Ok(U256Wrapper(u256))
    }
}
