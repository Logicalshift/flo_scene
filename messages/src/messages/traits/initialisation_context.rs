///
/// Context passed into a message initialisation instruction
///
pub trait InitialisationContext {
    ///
    /// Retrieves the specified type from the initialisation context
    ///
    fn get_init_context<TContextType: 'static>(&mut self) -> Option<&mut TContextType>;
}
