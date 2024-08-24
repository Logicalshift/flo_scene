pub (crate) mod scene;
pub (crate) mod scene_core;
pub (crate) mod subprogram_core;
pub (crate) mod process_core;
pub (crate) mod scene_context;
pub (crate) mod subprogram_id;
pub (crate) mod stream_id;
pub (crate) mod stream_source;
pub (crate) mod stream_target;
pub (crate) mod input_stream;
pub (crate) mod output_sink;
pub (crate) mod filter;
pub (crate) mod scene_message;
pub (crate) mod thread_stealer;
pub (crate) mod command_trait;
pub (crate) mod connect_result;

pub mod error;
pub mod programs;
pub mod commands;

pub use scene::*;
pub use scene_context::*;
pub use subprogram_id::*;
pub use stream_id::*;
pub use stream_source::*;
pub use stream_target::*;
pub use input_stream::*;
pub use output_sink::*;
pub use filter::*;
pub use scene_message::*;
pub use command_trait::*;
pub use connect_result::*;
pub use error::{ConnectionError, SceneSendError};

#[cfg(feature = "serde_support")]
mod serialization;

#[cfg(feature = "serde_support")]
pub use serialization::*;
