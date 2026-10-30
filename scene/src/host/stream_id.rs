use crate::host::error::*;
use crate::host::input_stream::*;
use crate::host::scene_message::*;
use crate::host::stream_target::*;
use crate::host::stream_type_functions::*;
use crate::host::subprogram_id::*;
use crate::host::programs::{SceneControl};

#[cfg(feature="guest_programs")]
use crate::guest::*;

use futures::prelude::*;
use futures::channel::mpsc::{Sender};

use std::any::*;
use std::hash::*;
use std::sync::*;

///
/// Identifies a stream produced by a subprogram 
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum StreamIdType {
    /// A stream identified by its message type
    MessageType,

    /// A stream sending data to a specific target
    Target(StreamTarget),
}

///
/// Identifies a stream produced by a subprogram 
///
#[derive(Clone, Eq, Debug)]
pub struct StreamId {
    stream_id_type:         StreamIdType,
    message_type_name:      &'static str,
    message_type:           TypeId,
    input_stream_core_type: TypeId,
}

impl PartialEq for StreamId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.stream_id_type == other.stream_id_type && self.message_type == other.message_type
    }
}

impl Hash for StreamId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.stream_id_type.hash(state);
        self.message_type.hash(state);
    }
}

impl StreamId {
    ///
    /// ID of a stream that generates a particular type of data
    ///
    pub fn with_message_type<TMessageType>() -> Self 
    where
        TMessageType: 'static + SceneMessage,
    {
        StreamId {
            stream_id_type:         StreamIdType::MessageType,
            message_type_name:      type_name::<TMessageType>(),
            message_type:           TypeId::of::<TMessageType>(),
            input_stream_core_type: TypeId::of::<Mutex<InputStreamCore<TMessageType>>>(),
        }
    }

    ///
    /// ID of a stream that is assigned to a particular target
    ///
    pub fn for_target(&self, target: impl Into<StreamTarget>) -> Self {
        StreamId {
            stream_id_type:         StreamIdType::Target(target.into()),
            message_type_name:      self.message_type_name,
            message_type:           self.message_type,
            input_stream_core_type: self.input_stream_core_type,
        }
    }

    ///
    /// Returns a stream ID that has no target program but is otherwise the same as the current stream
    ///
    pub fn as_message_type(&self) -> Self {
        StreamId {
            stream_id_type:         StreamIdType::MessageType,
            message_type_name:      self.message_type_name,
            message_type:           self.message_type,
            input_stream_core_type: self.input_stream_core_type,
        }
    }

    ///
    /// The target that was requested when this stream was created (Any for streams that just specify a message type)
    ///
    pub (crate) fn requested_target(&self) -> StreamTarget {
        match &self.stream_id_type {
            StreamIdType::MessageType       => StreamTarget::Any,
            StreamIdType::Target(target)    => target.clone(),
        }
    }

    ///
    /// None if this stream ID is not for a specific target, otherwise the program ID of the target that this stream is for
    ///
    pub fn target_program(&self) -> Option<SubProgramId> {
        match self.stream_id_type {
            StreamIdType::MessageType                                   => None,
            StreamIdType::Target(StreamTarget::Program(target_id))      => Some(target_id),
            StreamIdType::Target(StreamTarget::Filtered(_, target_id))  => Some(target_id),
            StreamIdType::Target(_)                                     => None,
        }
    }

    ///
    /// The type of message that can be sent to this stream
    ///
    pub fn message_type(&self) -> TypeId {
        self.message_type
    }

    ///
    /// The name of the Rust type that is the expected type name for this stream
    ///
    pub fn message_type_name(&self) -> String {
        self.message_type_name.into()
    }
}

mod serialization {
    use super::*;

    use serde::*;

    #[derive(Serialize, Deserialize)]
    enum SerializedStreamId {
        /// A known serializable type
        Serializable { type_name: String, target: Option<SubProgramId> },

        /// A Rust type, with the specified type name (note that this name may not be consistent between applications)
        RustType { type_name: String, target: Option<SubProgramId> },
    }

    impl Serialize for StreamId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let serialized = if let Some(serializable_name) = self.serialization_type_name() {
                SerializedStreamId::Serializable { type_name: serializable_name, target: self.target_program() }
            } else {
                SerializedStreamId::RustType { type_name: self.message_type_name(), target: self.target_program() }
            };

            serialized.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for StreamId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let stream_id = SerializedStreamId::deserialize(deserializer)?;

            match stream_id {
                SerializedStreamId::Serializable { type_name, target } => {
                    if let Some(stream_id) = StreamId::with_serialization_type(type_name) {
                        if let Some(target) = target {
                            Ok(stream_id.for_target(target))
                        } else {
                            Ok(stream_id)
                        }
                    } else {
                        // TODO: generate an error
                        todo!()
                    }
                }

                SerializedStreamId::RustType { type_name, target } => {
                    if let Some(stream_id) = StreamId::with_rust_type(type_name) {
                        if let Some(target) = target {
                            Ok(stream_id.for_target(target))
                        } else {
                            Ok(stream_id)
                        }
                    } else {
                        // TODO: generate an error
                        todo!()
                    }
                }
            }
        }
    }
}

pub trait StreamIdExt {
    #[cfg(all(feature="postcard", feature="guest_programs"))]
    fn run_host_subprogram_postcard(&self, program_id: SubProgramId, max_input_waiting: usize, actions: Sender<GuestAction>, results: impl 'static + Send + Stream<Item=GuestResult>) -> Result<SceneControl, ConnectionError>;
}

impl StreamIdExt for StreamId {
    ///
    /// Returns the scene control message required to start a guest host subprogram using the postcard encoding for messages 
    ///
    #[cfg(all(feature="postcard", feature="guest_programs"))]
    fn run_host_subprogram_postcard(&self, program_id: SubProgramId, max_input_waiting: usize, actions: Sender<GuestAction>, results: impl 'static + Send + Stream<Item=GuestResult>) -> Result<SceneControl, ConnectionError> {
        self.message_type().run_host_subprogram_postcard(program_id, max_input_waiting, actions, results)
    }
}
