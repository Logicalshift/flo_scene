use futures::prelude::*;
use futures::task;
use futures::task::{Poll, Waker, Context, ArcWake, waker_ref};

use std::pin::*;
use std::sync::*;

struct Reawakener(Mutex<Option<Waker>>);

///
/// 'Steals' the current thread to poll a future in immediate mode
///
/// The future is run immediately, and associated with the supplied waker for re-awakening
///
pub fn poll_thread_steal<TFuture>(future: Pin<&mut TFuture>, external_waker: Option<Waker>) -> Poll<TFuture::Output>
where
    TFuture: Future + ?Sized,
{
    // Create a futures context which will trigger the external awakener
    let waker       = task::waker(Arc::new(Reawakener(Mutex::new(external_waker))));
    let mut context = Context::from_waker(&waker);

    // Poll the future immediately
    future.poll(&mut context)
}

impl ArcWake for Reawakener {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let waker = arc_self.0.lock().unwrap().take();

        if let Some(waker) = waker {
            waker.wake()
        }
    }
}
