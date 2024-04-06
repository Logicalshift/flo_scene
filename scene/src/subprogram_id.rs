use uuid::{Uuid};
use once_cell::sync::{Lazy};

use std::collections::*;
use std::sync::*;

#[cfg(feature="serde_support")] use serde::*;

static IDS_FOR_NAMES: Lazy<RwLock<HashMap<String, usize>>> = Lazy::new(|| RwLock::new(HashMap::new()));
static NAMES_FOR_IDS: Lazy<RwLock<Vec<String>>>            = Lazy::new(|| RwLock::new(vec![]));

fn id_for_name(name: &str) -> usize {
    let id = { IDS_FOR_NAMES.read().unwrap().get(name).copied() };

    if let Some(id) = id {
        // ID already exists
        id
    } else {
        // Create a new ID
        let id = {
            let mut names_for_ids   = NAMES_FOR_IDS.write().unwrap();
            let id                  = names_for_ids.len();
            names_for_ids.push(name.into());

            id
        };

        // Store the mapping
        let mut ids_for_names = IDS_FOR_NAMES.write().unwrap();
        ids_for_names.insert(name.into(), id);

        id
    }
}

///
/// A unique identifier for a subprogram in a scene
///
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SubProgramId(SubProgramIdValue);

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
enum SubProgramIdValue {
    /// A subprogram identified with a well-known name
    Named(usize),

    /// A subprogram identified with a GUID
    Guid(Uuid),
}

impl SubProgramId {
    ///
    /// Creates a new unique subprogram id
    ///
    #[inline]
    #[allow(clippy::new_without_default)]   // As this isn't a default value, it's a *new* value, there's no default subprogram ID
    pub fn new() -> SubProgramId {
        SubProgramId(SubProgramIdValue::Guid(Uuid::new_v4()))
    }

    ///
    /// Creates a subprogram ID with a well-known name
    ///
    #[inline]
    pub fn called(name: &str) -> SubProgramId {
        SubProgramId(SubProgramIdValue::Named(id_for_name(name)))
    }
}
