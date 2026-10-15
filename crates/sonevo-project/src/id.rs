//! The identifiers that tie a song's parts together: random, so that two people adding to
//! the same song never take the same one; stable, so that renaming or moving breaks nothing.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Sixty-four random bits, written as sixteen hexadecimal digits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(u64);

impl Id {
    pub fn new() -> Id {
        let mut bytes = [0u8; 8];
        // the system's randomness; were it ever missing, the clock's still differs each time
        if getrandom::fill(&mut bytes).is_err() {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            bytes = (now.as_nanos() as u64).to_le_bytes();
        }
        Id(u64::from_le_bytes(bytes))
    }

    pub const fn from_raw(raw: u64) -> Id {
        Id(raw)
    }
}

impl Default for Id {
    fn default() -> Id {
        Id::new()
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({self})")
    }
}

impl std::str::FromStr for Id {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Id, Self::Err> {
        u64::from_str_radix(s, 16).map(Id)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Id, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(|_| serde::de::Error::custom(format!("{text} is not an id")))
    }
}
