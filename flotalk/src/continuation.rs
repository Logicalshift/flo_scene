use super::class::*;
use super::context::*;
use super::error::*;
use super::message::*;
use super::reference::*;
use super::releasable::*;
use super::value::*;

use futures::prelude::*;
use futures::task::{Poll, Context};

use std::mem;

///
/// Raw functions return a continuation, which specifies how a result may be retrieved
///
pub enum TalkContinuation<'a> {
    /// A value that's ready now
    Ready(TalkValue),

    /// A value that requires access to the context to compute, but which doesn't require awaiting a future
    Soon(Box<dyn 'a + Send + FnOnce(&mut TalkContext) -> TalkContinuation<'static>>),

    /// A value that is ready when a future completes
    Later(Box<dyn 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkContinuation<'static>>>),
}

impl<'a> TalkContinuation<'a> {
    ///
    /// Polls this continuation for a result
    ///
    #[inline]
    pub fn poll(&mut self, talk_context: &mut TalkContext, future_context: &mut Context) -> Poll<TalkValue> {
        use TalkContinuation::*;

        loop {
            let mut continuation = TalkContinuation::Ready(TalkValue::Nil);
            mem::swap(&mut continuation, self);

            match continuation {
                Ready(value)        => { return Poll::Ready(value); },
                Soon(soon)          => { *self = soon(talk_context); }

                Later(mut poll_fn)  => { 
                    let result = poll_fn(talk_context, future_context);
                    if let Poll::Ready(next) = result {
                        *self = next;
                    } else {
                        return Poll::Pending;
                    }
                },
            }
        }
    }

    ///
    /// Creates a 'TalkContinuation::Soon' from a function
    ///
    #[inline]
    pub fn soon(soon: impl 'a + Send + FnOnce(&mut TalkContext) -> TalkContinuation<'static>) -> Self {
        TalkContinuation::Soon(Box::new(soon))
    }

    ///
    /// Creates a 'TalkContinuation::Later' from a function returning a value
    ///
    #[inline]
    pub fn later_value(later: impl 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkValue>) -> Self {
        let mut later = later;
        TalkContinuation::Later(Box::new(move |talk_context, future_context| later(talk_context, future_context).map(|result| TalkContinuation::Ready(result))))
    }

    ///
    /// Creates a 'TalkContinuation::Later' from a function returning a further continuation
    ///
    #[inline]
    pub fn later_soon(later: impl 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkContinuation<'static>>) -> Self {
        TalkContinuation::Later(Box::new(later))
    }

    ///
    /// Creates a TalkContinuation from a future
    ///
    #[inline]
    pub fn future<TFuture>(future: TFuture) -> Self
    where
        TFuture: 'a + Send + Future<Output=TalkValue>,
    {
        let mut future = Box::pin(future);
        Self::later_value(move |_, ctxt| future.poll_unpin(ctxt))
    }

    ///
    /// Creates a TalkContinuation from a future
    ///
    #[inline]
    pub fn future_soon<TFuture>(future: TFuture) -> Self 
    where
        TFuture: 'a + Send + Future<Output=TalkContinuation<'static>>,
    {
        let mut future              = Box::pin(future);

        Self::later_soon(move |_talk_context, future_context| future.poll_unpin(future_context))
    }

    ///
    /// Once this continuation is finished, perform the specified function on the result
    ///
    #[inline]
    pub fn and_then(self, and_then: impl 'static + Send + FnOnce(TalkValue) -> TalkContinuation<'static>) -> TalkContinuation<'a> {
        match self {
            TalkContinuation::Ready(value)  => and_then(value),
            TalkContinuation::Soon(soon)    => TalkContinuation::Soon(Box::new(move |context| soon(context).and_then(and_then))),

            TalkContinuation::Later(later)  => {
                let mut later       = later;
                let mut and_then    = Some(and_then);

                TalkContinuation::Later(Box::new(move |talk_context, future_context| {
                    // Poll the 'later' value
                    let poll_result = later(talk_context, future_context);

                    if let Poll::Ready(value) = poll_result {
                        if let Some(and_then) = and_then.take() {
                            // When it finishes, call the 'and_then' function and update the 'later' value
                            Poll::Ready(value.and_then(and_then))
                        } else {
                            // Continuation has finished and the value is ready
                            Poll::Ready(value)
                        }
                    } else {
                        // Still pending
                        poll_result
                    }
                }))
            }
        }
    }

    ///
    /// Once this continuation is finished, perform the specified function on the result
    ///
    #[inline]
    pub fn and_then_soon(self, and_then: impl 'static + Send + FnOnce(TalkValue, &mut TalkContext) -> TalkContinuation<'static>) -> TalkContinuation<'a> {
        self.and_then(move |value| TalkContinuation::Soon(Box::new(move |context| and_then(value, context))))
    }

    ///
    /// Creates a continuation that reads the contents of a value (assuming it belongs to the specified allocator)
    ///
    #[inline]
    pub fn read_value<TClass, TOutput>(value: TalkValue, read_value: impl 'a + Send + FnOnce(&mut TClass::Data, &TalkContext) -> TOutput) -> TalkContinuation<'a>
    where
        TClass:     'static + TalkClassDefinition,
        TOutput:    Into<TalkContinuation<'static>>,
    {
        TalkContinuation::Soon(Box::new(move |talk_context| {
            match value {
                TalkValue::Reference(TalkReference(class_id, data_handle)) => {
                    // Get the callbacks for the class
                    let callbacks = talk_context.get_callbacks_mut(class_id);
                    if let Some(allocator) = callbacks.allocator::<TClass::Allocator>() {
                        // Retrieve the value of this data handle
                        let mut allocator   = allocator.lock().unwrap();
                        let data            = allocator.retrieve(data_handle);

                        // Call the callback to read the data
                        read_value(data, talk_context).into()
                    } else {
                        // Not the expected allocator
                        TalkError::UnexpectedClass.into()
                    }
                }

                TalkValue::Error(err)   => err.into(),
                _                       => TalkError::UnexpectedClass.into()
            }
        }))
    }
}

impl TalkContinuation<'static> {
    ///
    /// Causes an action to be run 'while' a condition is true. Returns the result of either the last run through the block or the `initial_return_value` if the block is never run
    ///
    /// If the condition or the block returns an error, then the result is that error.
    ///
    pub fn do_while(while_condition: impl 'static + Send + Fn(&mut TalkContext) -> TalkContinuation<'static>, action: impl 'static + Send + Fn(&mut TalkContext) -> TalkContinuation<'static>, initial_return_value: TalkValue) -> TalkContinuation<'static> {
        // Try to evaluate 'soon' if possible (avoiding returning a 'later' continuation)
        TalkContinuation::soon(move |talk_context| {
            use TalkContinuation::*;

            let mut last_result = initial_return_value;

            loop {
                // Try to get the result of evaluating the 'while' condition
                let mut while_continuation  = while_condition(talk_context);
                let while_result            = loop {
                    match while_continuation {
                        Ready(val)      => { break val; }
                        Soon(soon)      => { while_continuation = soon(talk_context); }
                        Later(later)    => {
                            // Put back in a continuation
                            let mut later       = TalkContinuation::Later(later);

                            let mut action          = Some(action);
                            let mut while_condition = Some(while_condition);
                            let mut last_result     = Some(last_result);

                            // Create a new continuation that performs this step of the while loop before re-entering do_while
                            return TalkContinuation::later_soon(move |talk_context, future_context| {
                                if let Poll::Ready(while_result) = later.poll(talk_context, future_context) {
                                    // Continue with the action if true, or stop on an error or any othe rvalue
                                    match while_result {
                                        TalkValue::Bool(true)   => { 
                                            // Done with the last result
                                            last_result.take().unwrap().release_in_context(talk_context);

                                            // Take ownership of the action and the condition
                                            let action              = action.take().unwrap();
                                            let while_condition     = while_condition.take().unwrap();

                                            // Create a continuation that runs the action and then continues the while loop
                                            let action_continuation = action(talk_context);
                                            let while_continuation  = action_continuation
                                                .and_then(move |action_result| {
                                                    match action_result {
                                                        TalkValue::Error(err)   => err.into(),
                                                        _                       => TalkContinuation::do_while(while_condition, action, action_result)
                                                    }
                                                });
                                            Poll::Ready(while_continuation) 
                                        }

                                        TalkValue::Error(err)   => { Poll::Ready(err.into()) }
                                        _                       => { Poll::Ready(last_result.take().unwrap().into()) }
                                    }
                                } else {
                                    // Stop looping
                                    Poll::Pending
                                }
                            })
                        }
                    }
                };

                // Check the result: stop if there's an error or the condition is any value other than true
                match while_result {
                    TalkValue::Error(err)   => { return err.into(); }
                    TalkValue::Bool(true)   => { }
                    _                       => { return last_result.into(); }
                }

                // Result was true, so we're replacing the last_result value
                last_result.release_in_context(talk_context);

                // Try to evaluate the action similarly
                let mut action_continuation = action(talk_context);
                let action_result           = loop {
                    match action_continuation {
                        Ready(val)      => { break val; }
                        Soon(soon)      => { action_continuation = soon(talk_context); }
                        Later(later)    => {
                            // Run the continuation then re-enter the while block
                            return TalkContinuation::Later(later)
                                .and_then(move |action_result| {
                                    match action_result {
                                        TalkValue::Error(err)   => err.into(),
                                        _                       => TalkContinuation::do_while(while_condition, action, action_result)
                                    }
                                })
                        }
                    }
                };

                // Stop if there's an error, otherwise set the last result so we can return it
                match action_result {
                    TalkValue::Error(err)   => { return err.into(); }
                    _                       => { last_result = action_result; }
                }
            }
        })
    }
}

impl<'a, T> From<T> for TalkContinuation<'a>
where
    T : Into<TalkValue>,
{
    #[inline]
    fn from(val: T) -> TalkContinuation<'a> {
        TalkContinuation::Ready(val.into())
    }
}

impl<'a, T, TErr> From<Result<T, TErr>> for TalkContinuation<'a>
where
    T: Into<TalkValue>,
    TErr: Into<TalkError>
{
    #[inline]
    fn from(val: Result<T, TErr>) -> TalkContinuation<'a> {
        match val {
            Ok(val)     => TalkContinuation::Ready(val.into()),
            Err(err)    => TalkContinuation::Ready(TalkValue::Error(err.into()))
        }
    }
}

impl<'a> From<TalkSendMessage> for TalkContinuation<'a> {
    #[inline]
    fn from(TalkSendMessage(target, message): TalkSendMessage) -> TalkContinuation<'a> {
        let mut target                  = target;
        let mut message                 = Some(message);

        TalkContinuation::soon(move |talk_context| target.take().send_message_in_context(message.take().unwrap(), talk_context))
    }
}
