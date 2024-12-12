use crate::host::scene_message::*;

use serde::*;

use std::borrow::{Cow};

///
/// Provides a detailed description of what a command does
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeCommandResponse {
    /// A summary of what the command does
    pub summary: String,

    /// Markdown that provides a description of what this command does
    pub help: Cow<'static, str>,
}

///
/// Request to describe a particular command
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeCommandRequest(pub String);

impl SceneMessage for DescribeCommandResponse { }
impl SceneMessage for DescribeCommandRequest { }
