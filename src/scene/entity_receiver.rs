use futures::prelude::*;
use futures::task;
use futures::task::{Poll, Context, Waker, ArcWake};

use std::pin::*;
use std::sync::*;
use std::sync::atomic::*;

///
/// Stream that keeps track of the total number of active entities for a scene
///
pub struct EntityReceiver<TStream> {
    stream:                 TStream,
    state:                  Arc<Mutex<EntityReceiverState>>,
}

struct EntityReceiverState {
    activation_count:       isize,
    active_entity_count:    Arc<AtomicIsize>,
    future_waker:           Option<Waker>,
}

struct EntityReceiverWaker {
    state: Arc<Mutex<EntityReceiverState>>,
}

impl<TStream> EntityReceiver<TStream>
where
    TStream:    Unpin + Stream
{
    /// Creates a new entity receiver
    pub fn new(stream: TStream, active_entity_count: &Arc<AtomicIsize>) -> EntityReceiver<TStream> {
        EntityReceiver {
            stream: stream,
            state:  Arc::new(Mutex::new(EntityReceiverState {
                activation_count:       0,
                active_entity_count:    Arc::clone(active_entity_count),
                future_waker:           None,
            })),
        }
    }
}

impl<TStream> Stream for EntityReceiver<TStream>
where
    TStream:    Unpin + Stream
{
    type Item = TStream::Item;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        // Create a new context and load it into the state. Retrieve the number of activations that occurred before the state was created
        let initial_activation_count = {
            // We read the initial activation count before polling in case we're woken up before this function returns
            let mut state           = self.state.lock().unwrap();
            state.future_waker  = Some(context.waker().clone());
            state.activation_count
        };

        // Create the context using our waker
        let waker           = Arc::new(EntityReceiverWaker { state: Arc::clone(&self.state) });
        let future_waker    = task::waker(waker);
        let mut context     = task::Context::from_waker(&future_waker);

        match self.stream.poll_next_unpin(&mut context) {
            Poll::Pending => {
                // Remove the activations from the active entity count (and our state)
                let mut state = self.state.lock().unwrap();

                state.activation_count -= initial_activation_count;
                state.active_entity_count.fetch_sub(initial_activation_count, Ordering::Relaxed);

                Poll::Pending
            }

            Poll::Ready(Some(msg)) => {
                Poll::Ready(Some(msg))
            }

            Poll::Ready(None) => {
                // Remove the activations from the active entity count (and our state)
                let mut state = self.state.lock().unwrap();

                // Entirely remove this from the active entity count
                state.future_waker      = None;
                state.activation_count  = 0;
                state.active_entity_count.fetch_sub(state.activation_count, Ordering::Relaxed);

                Poll::Ready(None)
            }
        }
    }
}

impl ArcWake for EntityReceiverWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // Fetch the state. If there's an active waker, increase the activation count
        let waker = {
            let mut state = arc_self.state.lock().unwrap();

            if state.future_waker.is_some() {
                state.activation_count += 1;
                state.active_entity_count.fetch_add(1, Ordering::Relaxed);
            }

            state.future_waker.take()
        };

        // Wake up the future (without the state locked)
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}
