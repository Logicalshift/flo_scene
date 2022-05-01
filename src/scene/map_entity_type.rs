use std::any::{Any};
use std::sync::*;

///
/// Provides a mapping function for a known entity type
///
pub (super) struct MapEntityType {
    /// Maps from a boxed 'Any' representing the source entity channel type to a boxed 'Any' representing the target entity channel type
    map_fn: Box<dyn Send + Any>,
}

impl MapEntityType {
    ///
    /// Creates a mapping from one type to another
    ///
    pub fn new<TSource, TTarget>() -> MapEntityType 
    where
        TSource: 'static + Send,
        TTarget: 'static + Send + From<TSource>,
    {
        // Box a function to convert from source to target
        let map_fn: Arc<dyn Sync + Send + Fn(TSource) -> TTarget> = Arc::new(|src: TSource| TTarget::from(src));

        // Box again to create the 'any' version of the function
        MapEntityType {
            map_fn: Box::new(map_fn)
        }
    }

    ///
    /// Returns the conversion function for mapping from a source to a target type
    ///
    pub fn conversion_function<TSource, TTarget>(&self) -> Option<Arc<dyn Sync + Send + Fn(TSource) -> TTarget>> 
    where
        TSource: 'static + Send,
        TTarget: 'static + Send + From<TSource>,
    {
        let conversion = self.map_fn.downcast_ref::<Arc<dyn Sync + Send + Fn(TSource) -> TTarget>>()?;

        Some(conversion.clone())
    }
}
