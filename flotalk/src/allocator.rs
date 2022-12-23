use super::class::*;
use super::context::*;
use super::reference::*;
use super::releasable::*;

use std::sync::*;

///
/// Typical implementation of the allocator for a TalkClass
///
/// This can be used in most places where a class allocator is required for a Rust type.
///
/// Create a new allocator:
///
/// ```
/// # use flo_talk::*;
/// let allocator = TalkStandardAllocator::<usize>::empty();
/// ```
///
/// Store a value in the allocator (for example, from a 'new' class method)
///
/// ```
/// # use flo_talk::*;
/// # let mut allocator = TalkStandardAllocator::<usize>::empty();
/// let handle = allocator.store(42);
/// # let handle_2 = allocator.store(43);
/// # assert!(handle.0 == 0);
/// # assert!(handle_2.0 == 1);
/// # assert!(allocator.retrieve(handle) == &mut 42);
/// # assert!(allocator.retrieve(handle_2) == &mut 43);
/// ```
///
pub struct TalkStandardAllocator<TValue> 
where
    TValue: TalkReleasable,
{
    /// The data store
    data: Vec<Option<TValue>>,

    /// Reference counts for each allocated item in the data store (data is dropped when the count reaches 0)
    reference_counts: Vec<usize>,

    /// Items in the data array that have been freed and are available for reallocation
    free_slots: Vec<usize>,
}

impl<TValue> TalkStandardAllocator<TValue>
where
    TValue: TalkReleasable,
{
    ///
    /// Creates an allocator with no values in it
    ///
    pub fn empty() -> TalkStandardAllocator<TValue> {
        TalkStandardAllocator {
            data:               vec![],
            reference_counts:   vec![],
            free_slots:         vec![],
        }
    }

    ///
    /// Stores a value in this allocator and returns a handle to it
    ///
    #[inline]
    pub fn store(&mut self, value: TValue) -> TalkDataHandle {
        if let Some(pos) = self.free_slots.pop() {
            self.data[pos]              = Some(value);
            self.reference_counts[pos]  = 1;

            TalkDataHandle(pos)
        } else {
            let pos = self.data.len();

            self.data.push(Some(value));
            self.reference_counts.push(1);

            TalkDataHandle(pos)
        }
    }
}

impl<TValue> TalkClassAllocator for TalkStandardAllocator<TValue> 
where
    TValue: Send + TalkReleasable,
{
    /// The type of data stored for this class
    type Data = TValue;

    ///
    /// Retrieves a reference to the data attached to a handle (panics if the handle has been released)
    ///
    #[inline]
    fn retrieve<'a>(&'a mut self, TalkDataHandle(pos): TalkDataHandle) -> &'a mut Self::Data {
        self.data[pos].as_mut().unwrap()
    }

    ///
    /// Adds to the reference count for a data handle
    ///
    #[inline]
    fn retain(allocator: &Arc<Mutex<Self>>, TalkDataHandle(pos): TalkDataHandle, _: &TalkContext) {
        let mut allocator = allocator.lock().unwrap();

        if allocator.reference_counts[pos] > 0 {
            allocator.reference_counts[pos] += 1;
        }
    }

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    #[inline]
    fn release(allocator: &Arc<Mutex<Self>>, TalkDataHandle(pos): TalkDataHandle, talk_context: &TalkContext) {
        let freed_value = {
            let mut allocator = allocator.lock().unwrap();

            if allocator.reference_counts[pos] > 0 {
                allocator.reference_counts[pos] -= 1;

                if allocator.reference_counts[pos] == 0 {
                    allocator.free_slots.push(pos);
                    allocator.data[pos].take()
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(freed_value) = freed_value {
            freed_value.release_in_context(talk_context);
        }
    }
}
