use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use ts_rs::TS;
use uuid::Uuid;

/// A canonical lowercase UUID used as a stable ID in saved schematics and engine results.
#[derive(Clone, Debug, Eq, Hash, PartialEq, TS)]
#[ts(type = "string", export, export_to = "StableId.ts")]
pub struct StableId(String);

impl StableId {
    pub fn parse(value: impl Into<String>) -> Result<Self, InvalidStableId> {
        let value = value.into();
        let parsed = Uuid::parse_str(&value).map_err(|_| InvalidStableId)?;

        if parsed.hyphenated().to_string() != value {
            return Err(InvalidStableId);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for StableId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for StableId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidStableId;

impl fmt::Display for InvalidStableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ID must be a canonical lowercase UUID")
    }
}

impl std::error::Error for InvalidStableId {}
