use std::any::{Any};
use std::sync::*;

///
/// Provides a mapping function for a known entity type
///
pub (super) struct MapIntoEntityType {
    /// Maps from a boxed 'Any' representing the source entity channel type to a boxed 'Any' representing the target entity channel type
    map_fn: Box<dyn Send + Any>,
}

impl MapIntoEntityType {
    ///
    /// Creates a mapping from one type to another
    ///
    pub fn new<TSource, TTarget>() -> MapIntoEntityType 
    where
        TSource: 'static + Send + Into<TTarget>,
        TTarget: 'static + Send,
    {
        // Box a function to convert from source to target
        let map_fn: Arc<dyn Sync + Send + Fn(Box<dyn Send + Any>) -> Option<TTarget>> = Arc::new(|src: Box<dyn Send + Any>| {
            let mut src = src;
            let src     = src.downcast_mut::<Option<TSource>>()?;
            let src     = src.take()?;

            Some(src.into())
        });

        // Box again to create the 'any' version of the function
        MapIntoEntityType {
            map_fn: Box::new(map_fn)
        }
    }

    ///
    /// Creates a mapping from one type to another using a mapping function
    ///
    pub fn with_map_fn<TSource, TTarget, TMapFn>(map_fn: TMapFn) -> MapIntoEntityType
    where
        TSource:    'static + Send,
        TTarget:    'static + Send,
        TMapFn:     'static + Send + Sync + Fn(TSource) -> TTarget,
    {
        // Box a function to convert from source to target
        let map_fn: Arc<dyn Sync + Send + Fn(Box<dyn Send + Any>) -> Option<TTarget>> = Arc::new(move |src: Box<dyn Send + Any>| {
            let mut src = src;
            let src     = src.downcast_mut::<Option<TSource>>()?;
            let src     = src.take()?;
            let tgt     = map_fn(src);

            Some(tgt)
        });

        // Box again to create the 'any' version of the function
        MapIntoEntityType {
            map_fn: Box::new(map_fn)
        }
    }

    ///
    /// Returns the conversion function for mapping from a source to a target type
    ///
    /// The value is a boxed 'Any' of `Option<TTarget>`. We use an option here as Box<Any> doesn't have a way of otherwise
    /// extracting the wrapped type
    ///
    pub fn conversion_function<TTarget>(&self) -> Option<Arc<dyn Sync + Send + Fn(Box<dyn Send + Any>) -> Option<TTarget>>> 
    where
        TTarget: 'static + Send,
    {
        let conversion = self.map_fn.downcast_ref::<Arc<dyn Sync + Send + Fn(Box<dyn Send + Any>) -> Option<TTarget>>>()?;

        Some(conversion.clone())
    }
}
