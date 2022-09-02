///
/// Error associated with an error binding a property
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BindingError {
    /// The requested binding was not available
    Missing,

    /// The request was dropped before the binding could be retrieved
    Abandoned,
}
