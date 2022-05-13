use futures::prelude::*;
use futures::task::{Poll, Context};

use std::pin::*;
use std::sync::*;
use std::sync::atomic::*;

///
/// Stream that keeps track of the total number of active entities for a scene
///
pub struct EntityReceiver<TStream> {
    stream:                 TStream,
    active_entity_count:    Arc<AtomicIsize>,
    is_active:              bool,
}

impl<TStream> EntityReceiver<TStream>
where
    TStream:    Unpin + Stream
{
    /// Creates a new entity receiver
    pub fn new(stream: TStream, active_entity_count: &Arc<AtomicIsize>) -> EntityReceiver<TStream> {
        EntityReceiver {
            stream:                 stream,
            active_entity_count:    Arc::clone(active_entity_count),
            is_active:              false,
        }
    }
}


impl<TStream> Stream for EntityReceiver<TStream>
where
    TStream:    Unpin + Stream
{
    type Item = TStream::Item;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        match self.stream.poll_next_unpin(context) {
            Poll::Pending => {
                // No longer active
                if self.is_active {
                    self.is_active = false;
                    self.active_entity_count.fetch_sub(1, Ordering::Relaxed);
                }

                Poll::Pending
            }

            Poll::Ready(Some(msg)) => {
                // Becomes active
                if !self.is_active {
                    self.is_active = true;
                    self.active_entity_count.fetch_add(1, Ordering::Relaxed);
                }

                Poll::Ready(Some(msg))
            }

            Poll::Ready(None) => {
                // No longer active
                if self.is_active {
                    self.is_active = false;
                    self.active_entity_count.fetch_sub(1, Ordering::Relaxed);
                }

                Poll::Ready(None)
            }
        }
    }
}
