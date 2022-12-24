use crate::allocator::*;
use crate::class::*;
use crate::context::*;
use crate::continuation::*;
use crate::error::*;
use crate::message::*;
use crate::reference::*;
use crate::releasable::*;
use crate::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use futures::prelude::*;
use futures::channel::mpsc;
use futures::lock;

use std::any::{TypeId};
use std::marker::{PhantomData};
use std::collections::{HashMap};
use std::sync::*;

/// Maps a receiver class for a particular stream type
static RECEIVER_CLASS: Lazy<Mutex<HashMap<TypeId, TalkClass>>> = Lazy::new(|| Mutex::new(HashMap::new()));

///
/// The sender class is a class that receives all its items from a stream
///
pub struct TalkReceiverClass<TStream>
where
    TStream:        'static + Send + Unpin + Stream<Item=TalkMessage>,
{
    receiver: PhantomData<Arc<lock::Mutex<TStream>>>,
}
