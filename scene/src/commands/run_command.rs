use crate::scene_message::*;
use crate::stream_target::*;
use crate::programs::*;

use std::marker::{PhantomData};

///
/// The RunCommand is a query request that will run a named command with a parameter, returning the result as a stream of responses to a target
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunCommand<TParameter, TResponse> {
    target:     StreamTarget,
    name:       String,
    parameter:  TParameter,
    response:   PhantomData<TResponse>,
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

impl<TParameter, TResponse> SceneMessage for RunCommand<TParameter, TResponse>
where
    TParameter: Unpin + Send,
    TResponse:  Unpin + Send
{
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