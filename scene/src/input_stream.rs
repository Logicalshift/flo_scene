use crate::{SubProgramId};

use futures::prelude::*;
use futures::task::{Waker, Poll};

use std::collections::*;
use std::sync::*;

///
/// The input stream core is a shareable part of an input stream for a program
///
pub (crate) struct InputStreamCore<TMessage> {
    /// The subprogram that this stream belongs to
    program_id: SubProgramId,

    /// The maximum number of waiting messages for this input stream
    max_waiting: usize,

    /// Messages waiting to be delivered
    waiting_messages: VecDeque<TMessage>,

    /// A waker for the future that is waiting for the next message in this stream
    waker: Option<Waker>,

    /// True if this stream is closed (because the subprogram is ending)
    closed: bool,
}

///
/// An input stream for a subprogram
///
pub struct InputStream<TMessage> {
    core: Arc<Mutex<InputStreamCore<TMessage>>>,
}

impl<TMessage> InputStream<TMessage> {
    pub (crate) fn new(program_id: SubProgramId, max_waiting: usize) -> Self {
        let core = InputStreamCore {
            program_id:         program_id,
            max_waiting:        max_waiting,
            waiting_messages:   VecDeque::new(),
            waker:              None,
            closed:             false,
        };

        InputStream {
            core: Arc::new(Mutex::new(core))
        }
    }
}

impl<TMessage> Stream for InputStream<TMessage> {
    type Item=TMessage;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        let mut core = self.core.lock().unwrap();

        if let Some(message) = core.waiting_messages.pop_front() {
            Poll::Ready(Some(message))
        } else if core.closed {
            Poll::Ready(None)
        } else {
            core.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
