use crate::scene_context::*;
use crate::scene_message::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::stream::{BoxStream};

use std::marker::{PhantomData};

///
/// Commands are spawnable tasks that carry out actions on behalf of a parent subprogram. A command can send multiple messages
/// to different targets and also can return a 'standard' output stream to to the subprogram that spawned it.
///
pub trait Command {
    type TInput:  'static + Send;
    type TOutput: 'static + SceneMessage;

    fn run(&self, input: impl 'static + Send + Stream<Item=Self::TInput>, context: SceneContext) -> impl 'static + Send + Future<Output=()>;
}

///
/// Basic type of a command that runs a function
///
pub struct FnCommand<TInput, TOutput>(PhantomData<TOutput>, Box<dyn 'static + Send + Sync + Fn(BoxStream<'static, TInput>, SceneContext) -> BoxFuture<'static, ()>>);

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
        FnCommand(PhantomData, Box::new(move |stream, context| action(stream, context).boxed()))
    }
}

impl<TInput, TOutput> Command for FnCommand<TInput, TOutput> 
where
    TInput:     'static + Send,
    TOutput:    'static + SceneMessage
{
    type TInput     = TInput;
    type TOutput    = TOutput;

    #[inline]
    fn run(&self, input: impl 'static + Send + Stream<Item=Self::TInput>, context: SceneContext) -> impl 'static + Send + Future<Output=()> {
        self.1(input.boxed(), context)
    }
}