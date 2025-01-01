use serde::*;

///
/// A filter is a way to convert from a stream of one message type to another, and a filter
/// handle references a predefined filter.
///
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(Debug)]        // TODO
pub struct FilterHandle(pub usize);
