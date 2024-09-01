use super::guest_message::*;

use futures::future::{BoxFuture};
use futures::task::{Waker};

use std::collections::{VecDeque};
use std::marker::{PhantomData};
use std::sync::*;

///
/// The input stream core is used in
///
pub (crate) struct GuestInputStreamCore {
    /// Messages waiting in this input stream
    waiting: VecDeque<Vec<u8>>,

    /// Waker for the future for this input stream
    waker: Option<Waker>,
}

///
/// A guest input stream works with the reads deserialized messages from the host side
///
pub struct GuestInputStream<TMessageType: GuestSceneMessage> {
    /// The core is shared with the runtime for managing the input stream
    core: Arc<Mutex<GuestInputStreamCore>>,

    /// Phantom data, what the waiting messages are decoded as
    decode_as: PhantomData<TMessageType>,
}
