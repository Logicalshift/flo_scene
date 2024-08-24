use crate::command_trait::*;
use crate::scene_context::*;
use crate::scene_message::*;

use futures::prelude::*;

use std::marker::{PhantomData};

///
/// The read command just sends its input straight to its output (this is useful to read the result of a query when used with `spawn_query`)
///
pub struct ReadCommand<TInputType>(PhantomData<TInputType>);

impl<TInputType> Default for ReadCommand<TInputType> {
    fn default() -> Self {
        ReadCommand(PhantomData)
    }
}

impl<TInputType> Clone for ReadCommand<TInputType> {
    fn clone(&self) -> Self {
        ReadCommand(PhantomData)
    }
}

impl<TInputType> Command for ReadCommand<TInputType> 
where
    TInputType: 'static + SceneMessage
{
    type Input = TInputType;
    type Output = TInputType;

    fn run<'a>(&'a self, input: impl 'static + Send + Stream<Item=Self::Input>, context: SceneContext) -> impl 'a + Send + Future<Output=()> {
        async move {
            if let Ok(output) = context.send(()) {
                let mut input   = Box::pin(input);
                let mut output  = output;

                while let Some(next) = input.next().await {
                    if let Err(_) = output.send(next).await {
                        break;
                    }
                }
            }
        }
    }
}
