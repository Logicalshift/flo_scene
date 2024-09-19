use crate::scene_message::*;
use crate::stream_target::*;
use crate::programs::*;

use serde::*;
use serde::de::{Error as DeError};
use serde::ser::{Error as SeError};

use std::marker::{PhantomData};
use std::sync::*;

///
/// The RunCommand is a query request that will run a named command with a parameter, returning the result as a stream of responses to a target
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunCommand<TParameter, TResponse> {
    /// Where the responses to the command should be sent
    target:     StreamTarget,

    /// The name of the command to run
    name:       String,

    /// Data to send to the command
    parameter:  TParameter,

    /// Phantom data for the response type
    response:   PhantomData<Mutex<TResponse>>,
}

impl<TParameter, TResponse> RunCommand<TParameter, TResponse>
where
    TParameter: Unpin + Send,
    TResponse:  Unpin + Send
{
    ///
    /// Creates a new 'run command' request. The command with the specified name will be run, and will send its response to the target.
    ///
    pub fn new(target: impl Into<StreamTarget>, name: impl Into<String>, parameter: impl Into<TParameter>) -> Self {
        Self {
            target:     target.into(),
            name:       name.into(),
            parameter:  parameter.into(),
            response:   PhantomData,
        }
    }

    ///
    /// Returns the program that the response to the command should be setn to
    ///
    pub fn target(&self) -> StreamTarget {
        self.target.clone()
    }

    ///
    /// The name of the command that is to be run
    ///
    pub fn name(&self) -> &str {
        &self.name
    }

    ///
    /// The parameter to the command
    ///
    pub fn parameter(&self) -> &TParameter {
        &self.parameter
    }
}

impl<TParameter, TResponse> Serialize for RunCommand<TParameter, TResponse> {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        Err(S::Error::custom("RunCommand cannot be serialized"))
    }
}

impl<'a, TParameter, TResponse> Deserialize<'a> for RunCommand<TParameter, TResponse> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a> 
    {
        Err(D::Error::custom("RunCommand cannot be serialized"))
    }
}

impl<TParameter, TResponse> SceneMessage for RunCommand<TParameter, TResponse>
where
    TParameter: Unpin + Send,
    TResponse:  Unpin + Send
{
    fn serializable() -> bool { false }
}

impl<TParameter, TResponse> QueryRequest for RunCommand<TParameter, TResponse> 
where
    TParameter: Unpin + Send,
    TResponse:  Unpin + Send
{
    type ResponseData = TResponse;

    fn with_new_target(self, new_target: StreamTarget) -> Self {
        RunCommand {
            target:     new_target,
            name:       self.name,
            parameter:  self.parameter,
            response:   PhantomData
        }
    }
}