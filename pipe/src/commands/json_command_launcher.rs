use super::command_stream::*;
use super::json_command::*;

use flo_scene::*;
use flo_scene::commands::*;

use futures::prelude::*;

use serde::*;
use serde_json;

use std::sync::*;

///
/// Extensions for the CommandLauncher which adds some convenience functions for creating JSON commands
///
pub trait JsonCommandLauncherExt {
    ///
    /// Creates a new, empty JSON command launcher
    ///
    fn json() -> Self;

    fn with_json_command<TParameter, TFuture>(self, command_name: impl Into<String>, command: impl 'static + Send + Sync + Fn(TParameter, SceneContext) -> TFuture) -> Self
    where
        TFuture:            'static + Send + Future,
        TFuture::Output:    'static + TryInto<CommandResponse>,
        TParameter:         'static + Send + for<'a> Deserialize<'a>;
}

impl JsonCommandLauncherExt for CommandLauncher<JsonParameter, CommandResponse> {
    fn json() -> Self {
        Self::empty()
    }

    fn with_json_command<TParameter, TFuture>(self, command_name: impl Into<String>, command: impl 'static + Send + Sync + Fn(TParameter, SceneContext) -> TFuture) -> Self
    where
        TFuture:            'static + Send + Future,
        TFuture::Output:    'static + TryInto<CommandResponse>,
        TParameter:         'static + Send + for<'a> Deserialize<'a>,
    {
        let command = Arc::new(command);

        self.with_command(command_name, move |json, context|
            {
                let command     = Arc::clone(&command);
                let parameter   = TParameter::deserialize(&json.value);

                async move {
                    // Connect to the output stream to generate the response
                    let response     = context.send::<CommandResponse>(());
                    let mut response = if let Ok(response) = response { response } else { return; };

                    if let Ok(parameter) = parameter {
                        // Invoke the command to get the response
                        let command_result = command(parameter, context).await;

                        if let Ok(command_result) = command_result.try_into().map_err(|_| ()) {
                            response.send(command_result).await.ok();
                        } else {
                            // Could not serialize the result
                            response.send(CommandError::CannotConvertResponse.into()).await.ok();
                        }
                    } else {
                        // Could not deserialize parameter
                        response.send(CommandError::IncorrectParameterFormat.into()).await.ok();
                    }
                }
            })
    }
}
