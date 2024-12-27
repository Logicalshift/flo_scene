use crate::guest_types::*;
use crate::imports::*;
use crate::util::*;

use ::serde::*;
use ::serde::de::*;
use ::serde::ser::{Error};
use uuid::*;
use once_cell::race::{OnceBox};

use alloc::boxed::*;
use alloc::string::*;
use alloc::vec::*;

use core::fmt;
use core::fmt::{Debug, Formatter};

// There aren't many options for no-std 'once' type initialisations (OnceLock is not available)

static IDS_FOR_NAMES: OnceBox<Shared<OrderedVec<String, SubProgramNameId>>> = OnceBox::new();
static NAMES_FOR_IDS: OnceBox<Shared<Vec<String>>>                          = OnceBox::new();

fn id_for_name(name: &str) -> SubProgramNameId {
    let ids_for_names   = IDS_FOR_NAMES.get_or_init(|| Box::new(share(OrderedVec::new())));
    let names_for_ids   = NAMES_FOR_IDS.get_or_init(|| Box::new(share(Vec::new())));

    let id = with_shared(&*ids_for_names, |ids_for_names| ids_for_names.get(name).copied());

    if let Some(id) = id {
        // ID already exists
        id
    } else {
        // Create a new ID and associate it with this name
        let id = with_shared(&*names_for_ids, |names_for_ids| {
            let id = names_for_ids.len();
            names_for_ids.push(name.into());

            id
        });
        let id = SubProgramNameId(id);

        // Store the mapping
        with_shared(&*ids_for_names, |ids_for_names| {
            if let Some(existing_id) = ids_for_names.get(name.into()).copied() {
                // Lost a race: ID was previously assigned in another thread, so use the ID that was assigned first
                // (We do assign an extra ID for this name, but we'll never use it)
                existing_id
            } else {
                ids_for_names.insert(name.into(), id);

                id
            }
        })
    }
}

fn name_for_id(id: SubProgramNameId) -> Option<String> {
    if let Some(names_for_ids) = NAMES_FOR_IDS.get() {
        with_shared(&*names_for_ids, |names_for_ids| names_for_ids.get(id.0).cloned())
    } else {
        None
    }
}

///
/// A unique identifier for a subprogram in a scene
///
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[derive(Serialize, Deserialize)]
pub struct SubProgramId(SubProgramIdValue);

///
/// A subprogram name ID
///
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
struct SubProgramNameId(usize);

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
#[derive(Serialize, Deserialize)]
enum SubProgramIdValue {
    /// A subprogram identified with a well-known name
    Named(SubProgramNameId),

    /// A subprogram identified with a GUID
    Guid(Uuid),

    /// A task created by a named subprogram. The second 'usize' value is a unique serial number for this task
    ///
    /// Tasks differ from subprograms in that they have a limited lifespan and read an input stream specified at creation
    NamedTask(SubProgramNameId, usize),

    /// A task created by a GUID subprogram. The 'usize' value is a unique serial number for this task
    ///
    /// Tasks differ from subprograms in that they have a limited lifespan and read an input stream specified at creation
    GuidTask(Uuid, usize),
}

impl SubProgramId {
    ///
    /// Creates a new unique subprogram id
    ///
    #[inline]
    #[allow(clippy::new_without_default)]   // As this isn't a default value, it's a *new* value, there's no default subprogram ID
    pub fn new() -> SubProgramId {
        SubProgramId(SubProgramIdValue::Guid(new_uuid()))
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

            SubProgramIdValue::Guid(guid)               |
            SubProgramIdValue::GuidTask(guid, _)        => SubProgramId(SubProgramIdValue::GuidTask(guid, command_sequence_number)),
        }
    }

    ///
    /// Returns true if this program is a subtask of another program
    ///
    pub fn is_subtask(&self) -> bool {
        match self.0 {
            SubProgramIdValue::Named(_) | SubProgramIdValue::Guid(_)                => false,
            SubProgramIdValue::NamedTask(_, _) | SubProgramIdValue::GuidTask(_, _)  => true,
        }
    }

    ///
    /// If this is a subtask, then return the ID of the program that laucnhed it
    ///
    pub fn parent_subprogram(&self) -> Option<SubProgramId> {
        match &self.0 {
            SubProgramIdValue::Named(_) | SubProgramIdValue::Guid(_)    => None,
            SubProgramIdValue::NamedTask(parent, _)                     => Some(SubProgramId(SubProgramIdValue::Named(*parent))),
            SubProgramIdValue::GuidTask(parent, _)                      => Some(SubProgramId(SubProgramIdValue::Guid(*parent))),
        }
    }

    ///
    /// Creates a string name for this subprogram
    ///
    pub fn to_string(&self) -> String {
        match &self.0 {
            SubProgramIdValue::Guid(guid)                   => guid.to_string(),
            SubProgramIdValue::Named(name_idx)              => name_for_id(*name_idx).unwrap_or_else(|| "<NO NAME>".to_string()),
            SubProgramIdValue::GuidTask(guid, task_idx)     => guid.to_string() + ".task(" + &task_idx.to_string() + ")",
            SubProgramIdValue::NamedTask(name_idx,task_idx) => name_for_id(*name_idx).unwrap_or_else(|| "<NO NAME>".to_string()) + ".task(" + &task_idx.to_string() + ")",
        }
    }

    ///
    /// If this is not a named program, returns the UUID of the owning subprogram
    ///
    pub fn to_uuid(&self) -> Option<Uuid> {
        match &self.0 {
            SubProgramIdValue::Guid(guid)           => Some(guid.clone()),
            SubProgramIdValue::GuidTask(guid, _)    => Some(guid.clone()),
            _                                       => None,
        }
    }

    ///
    /// If this is a named program, returns the name of the owning subprogram
    ///
    pub fn to_name(&self) -> Option<String> {
        match &self.0 {
            SubProgramIdValue::Named(name_idx)          |
            SubProgramIdValue::NamedTask(name_idx, _)   => name_for_id(*name_idx),
            _                                           => None,
        }
    }
}

impl Debug for SubProgramId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.0 {
            SubProgramIdValue::Guid(guid)                   => f.write_str(&("SubProgramId(".to_string() + &guid.to_string() + ")")),
            SubProgramIdValue::Named(name_idx)              => f.write_str(&("SubProgramId::called(\"".to_string() + &name_for_id(*name_idx).unwrap() + "\" <" + &name_idx.0.to_string() + ">)")),
            SubProgramIdValue::GuidTask(guid, task_idx)     => f.write_str(&("SubProgramId(".to_string() + &guid.to_string() + ").task(" + &task_idx.to_string() + ")")),
            SubProgramIdValue::NamedTask(name_idx,task_idx) => f.write_str(&("SubProgramId::called(\"".to_string() + &name_for_id(*name_idx).unwrap() + "\").task(" + &task_idx.to_string() + ")")),
        }
    }
}

impl Serialize for SubProgramNameId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        let name_string = name_for_id(*self);
        if let Some(name_string) = name_string {
            serializer.serialize_str(&name_string)
        } else {
            Err(S::Error::custom("No name"))
        }
    }
}

impl<'de> Deserialize<'de> for SubProgramNameId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de> 
    {
        struct StrVisitor;
        impl<'de> Visitor<'de> for StrVisitor {
            type Value = String;

            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                formatter.write_str("A string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value.to_string())
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value)
            }
        }

        let name_string = deserializer.deserialize_str(StrVisitor)?;
        Ok(id_for_name(&name_string))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use serde_json::{json};

    #[test]
    pub fn serialize_name() {
        let subprogram_id   = id_for_name("test");
        let json_name       = subprogram_id.serialize(serde_json::value::Serializer).unwrap();

        assert!(json_name == json!["test"]);
    }

    #[test]
    pub fn deserialize_name() {
        let deserialized_name = SubProgramNameId::deserialize(json!["another_test"]).unwrap();

        assert!(deserialized_name == id_for_name("another_test"));
    }
}
