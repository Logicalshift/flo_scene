use crate::entity_id::*;
use crate::error::*;
use crate::scene::scene_core::*;

use ::desync::*;

use std::mem;
use std::sync::*;
use std::cell::{RefCell};

thread_local! {
    pub static CURRENT_CONTEXT: RefCell<Option<Arc<SceneContext>>> = RefCell::new(None);
}

///
/// Used to restore the context after a `with_context` call returns
///
struct DropContext {
    previous_context: Option<Arc<SceneContext>>
}

///
/// The context allows for communication with the current scene
///
pub struct SceneContext {
    /// The component that's executing code on the current thread, or none for things like default actions
    component: Option<EntityId>,

    /// The core of the scene that the component is a part of
    scene_core: Arc<Desync<SceneCore>>,
}

impl SceneContext {
    ///
    /// Retrieves the active scene context (or a context error if one is available)
    ///
    pub fn current() -> Result<Arc<SceneContext>, SceneContextError> {
        CURRENT_CONTEXT
            .try_with(|ctxt| ctxt.borrow().clone())?
            .ok_or(SceneContextError::NoCurrentScene)
    }

    ///
    /// Evaluates a function within a particular scene context
    ///
    /// This is typically done automatically when running the runtimes for entities, but this can be used if if's ever necessary to
    /// artificially change contexts (eg: if an entity spawns its own thread, or in an independent runtime)
    ///
    #[inline]
    pub fn with_context<TFn, TResult>(new_context: &Arc<SceneContext>, in_context: TFn) -> Result<TResult, SceneContextError>
    where
        TFn: FnOnce() -> TResult
    {
        let new_context = Arc::clone(new_context);

        // When the function returns, reset the context
        let last_context = DropContext {
            previous_context: CURRENT_CONTEXT.try_with(|ctxt| ctxt.borrow().clone())?,
        };

        // Set the updated context
        CURRENT_CONTEXT.try_with(move |ctxt| *(ctxt.borrow_mut()) = Some(new_context))?;

        // Call the function with the new context
        let result = in_context();

        // Restore the context
        mem::drop(last_context);

        Ok(result)
    }

    ///
    /// Returns the component that this context is for
    ///
    pub fn entity_id(&self) -> Option<EntityId> {
        self.component
    }
}

impl Drop for DropContext {
    fn drop(&mut self) {
        let previous_context = self.previous_context.take();
        CURRENT_CONTEXT.try_with(move |ctxt| *(ctxt.borrow_mut()) = previous_context).ok();
    }
}
