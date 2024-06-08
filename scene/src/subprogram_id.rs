use uuid::{Uuid};
use once_cell::sync::{OnceCell, Lazy};

use std::ops::{Deref};
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

    /// A task created by a named subprogram. The second 'usize' value is a unique serial number for this task
    ///
    /// Tasks differ from subprograms in that they have a limited lifespan and read an input stream specified at creation
    NamedTask(usize, usize),

    /// A task created by a GUID subprogram. The 'usize' value is a unique serial number for this task
    ///
    /// Tasks differ from subprograms in that they have a limited lifespan and read an input stream specified at creation
    GuidTask(Uuid, usize),
}

///
/// A static subprogram ID can be used to declare a subprogram ID in a static variable
///
pub struct StaticSubProgramId(&'static str, OnceCell<SubProgramId>);

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

    ///
    /// Creates a command subprogram ID (with a particular sequence number)
    ///
    pub (crate) fn with_command_id(&self, command_sequence_number: usize) -> SubProgramId {
        match self.0 {
            SubProgramIdValue::Named(name_num)          |
            SubProgramIdValue::NamedTask(name_num, _)   => SubProgramId(SubProgramIdValue::NamedTask(name_num, command_sequence_number)),

            SubProgramIdValue::Guid(guid)               => SubProgramId(SubProgramIdValue::GuidTask(guid, command_sequence_number)),
            SubProgramIdValue::GuidTask(guid, _)        => SubProgramId(SubProgramIdValue::GuidTask(guid, command_sequence_number+1)),
        }
    }
}

impl StaticSubProgramId {
    ///
    /// Creates a subprogram ID with a well-known name
    ///
    #[inline]
    pub const fn called(name: &'static str) -> StaticSubProgramId {
        StaticSubProgramId(name, OnceCell::new())
    }
}

impl Deref for StaticSubProgramId {
    type Target = SubProgramId;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.1.get()
            .unwrap_or_else(|| {
                let subprogram = SubProgramId::called(self.0);
                self.1.set(subprogram).ok();
                self.1.get().unwrap()
            })
    }
}