use serde::*;

use core::any::{Any};
use core::fmt::{Debug};
use core::hash::{Hash, Hasher};
use alloc::sync::{Arc};

// TODO: rename FilterHandle, it's just a filter now
// TODO: try to move StreamTarget/FilterHandle into Scene from Guest if we can (the public fields here are just to support having the implementation in the other crate, which is awkward)

///
/// A filter is a way to convert from a stream of one message type to another, and a filter
/// handle references a predefined filter.
///
#[derive(Clone)]
pub struct FilterHandle {
    /// Internal data that defines the filter for the scene
    pub data: Arc<dyn Send + Sync + Any>,
    
    /// Serial number for the filter (used to determine if two filters represent the same underlying object)
    pub serial: usize,
}

impl Debug for FilterHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FilterHandle(")?;
        f.write_str(&self.serial.to_string())?;
        f.write_str(")")?;

        Ok(())
    }
}

impl PartialEq for FilterHandle {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
    }
}

impl Eq for FilterHandle {
}

impl Hash for FilterHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.serial.hash(state)
    }
}

impl Serialize for FilterHandle {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        use serde::ser::{Error};
        Err(S::Error::custom("Filters cannot be serialized"))
    }
}

impl<'de> Deserialize<'de> for FilterHandle {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de> 
    {
        use serde::de::{Error};
        Err(D::Error::custom("Filters cannot be deserialized"))
    }
}

