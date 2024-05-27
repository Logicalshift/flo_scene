use crate::command_trait::*;
use crate::scene_context::*;
use crate::scene_message::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::stream::{BoxStream};

use std::marker::{PhantomData};
use std::sync::*;

///
/// Basic type of a command that runs a function
///
#[derive(Clone)]
pub struct FnCommand<TInput, TOutput>(PhantomData<TOutput>, Arc<dyn 'static + Send + Sync + Fn(BoxStream<'static, TInput>, SceneContext) -> BoxFuture<'static, ()>>);

impl<TInput, TOutput> FnCommand<TInput, TOutput>
where
    TInput:     'static + Send,
    TOutput:    'static + SceneMessage
{
    ///
    /// Creates a new FnCommand with an implementing function
    ///
    pub fn new<TFuture>(action: impl 'static + Send + Sync + Fn(BoxStream<'static, TInput>, SceneContext) -> TFuture) -> Self 
    where
        TFuture: 'static + Send + Future<Output=()>,
    {
        FnCommand(PhantomData, Arc::new(move |stream, context| action(stream, context).boxed()))
    }
}

impl<TInput, TOutput> Command for FnCommand<TInput, TOutput> 
where
    TInput:     'static + Send,
    TOutput:    'static + SceneMessage
{
    type Input  = TInput;
    type Output = TOutput;

    #[inline]
    fn run(&self, input: impl 'static + Send + Stream<Item=Self::Input>, context: SceneContext) -> impl 'static + Send + Future<Output=()> {
        self.1(input.boxed(), context)
    }
}
