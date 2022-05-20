
///
/// Errors relating to managing background futures for an entity
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EntityFutureError {
    /// Can't create a backgroudn future for an entity that's not running/doesn't exist
    NoSuchEntity,
}
