//!
//! # Welcome to flo_scene
//!
//! `flo_scene` is a framework that can be used to compose small programs into larger programs, by structuring them
//! as entities that communicate by exchanging messages.
//!
//! # Examples
//!
//! Create the default scene (which is set up to have the standard set of components):
//!
//! ```
//! # use flo_scene::*;
//! let scene = Scene::default();
//! ```
//!
//! Run all of the entities in a scene:
//!
//! ```
//! # use flo_scene::*;
//! # let scene = Scene::default();
//! # scene.create_entity::<(), _, _>(EntityId::new(), |context, _| async move {
//! #   context.send_to(SCENE_CONTROL).unwrap().send(SceneControlRequest::StopScene).await.unwrap();
//! # }).unwrap();
//! use futures::executor;
//! executor::block_on(async move { scene.run().await });
//! ```
//!
//! Create a new entity in a scene:
//!
//! ```
//! # use flo_scene::*;
//! # use futures::prelude::*;
//! # let scene = Scene::empty();
//! let context = scene.context();
//!
//! context.create_entity(EXAMPLE, move |_context, mut requests| {
//!     async move {
//!         while let Some(request) = requests.next().await {
//!             match request {
//!                 ExampleRequest::Example => { println!("Example!"); }
//!             }
//!         }
//!     }
//! }).unwrap();
//! ```
//!
//! Send messages to an entity within a scene:
//!
//! ```
//! # use flo_scene::*;
//! # use futures::executor;
//! # let scene = Scene::default();
//! # let context = scene.context();
//! let mut channel = context.send_to::<ExampleRequest>(EXAMPLE).unwrap();
//! executor::block_on(async { 
//!   channel.send(ExampleRequest::Example).await.unwrap();
//! });
//! ```
//!
//! Use a recipe to execute a pre-set sequence of requests (with expected responses):
//!
//! ```
//! # use flo_scene::*;
//! # use futures::executor;
//! # use std::thread;
//! # let scene = Scene::default();
//! # let scene_context = scene.context();
//! # thread::spawn(move || executor::block_on(scene.run()));
//! let recipe = Recipe::new()
//!     .expect(vec![Heartbeat])
//!     .after_sending_messages(HEARTBEAT, |heartbeat_channel| vec![HeartbeatRequest::RequestHeartbeat(heartbeat_channel)])
//!     .alongside_generated_messages(EXAMPLE, || vec![ExampleRequest::Example]);
//!
//! executor::block_on(async { recipe.run(&scene_context).await.unwrap(); });
//! ```
//!
//! # Concepts
//!
//! `flo_scene` provides a runtime designed to make it easier to build large systems by amalgamating small programs. It takes
//! inspiration from the early object-oriented languages such as Simula and SmallTalk which were more about co-routines and
//! message passing than methods and inheritance. Rust is well suited for this style of programming.
//!
//! `flo_scene` calls these small programs 'entities', partly to distinguish them from the modern concept of an object and
//! partly because it also contains a properties system that gives it many of the features of an Entity Component System.
//!
//! Building large systems from small entities provides a way to manage dependencies in large projects: any two entities 
//! only need to know about the messages that can be sent between them and don't need to directly depend on each other.
//! Entities can query their context or the `ENTITY_REGISTRY` to discover each other, rather than needing to have
//! direct knowledge of each other.
//!
//! Another advantage is that individual entities tend to stand alone, which can simplify development and testing.
//!
//! Representing communications as messages has several advantages: the messages themselves can be reprocessed using
//! functions from the `futures` library (such as `map()`), messages can be serialized and replayed such as in the recipe
//! example above, and messages are much easier to send from a scripting language without needing a native call interface.
//!
//! A properties system is provided to allow entities to attach properties to themselves and other entities. This uses the
//! `flo_binding` library, so these values can be fully reactive computed properties as well as manually updated constants.
//!

#[cfg(feature="properties")] #[macro_use] extern crate lazy_static;

mod error;
mod scene;
mod entity_id;
mod entity_channel;
mod immediate_entity_channel;
mod ergonomics;
mod simple_entity_channel;
mod any_entity_channel;
mod mapped_entity_channel;
mod convert_entity_channel;
mod expected_entity_channel;
mod context;
mod standard_components;
mod panic_entity_channel;

pub use self::error::*;
pub use self::scene::*;
pub use self::entity_id::*;
pub use self::entity_channel::*;
pub use self::immediate_entity_channel::*;
pub use self::ergonomics::*;
pub use self::simple_entity_channel::*;
pub use self::mapped_entity_channel::*;
pub use self::convert_entity_channel::*;
pub use self::panic_entity_channel::*;
pub use self::any_entity_channel::*;
pub use self::expected_entity_channel::*;
pub use self::context::*;
pub use self::standard_components::*;

#[cfg(feature="test-scene")] pub use self::ergonomics::test;
#[cfg(feature="properties")] pub use flo_binding as binding;
#[cfg(feature="properties")] pub use flo_rope as rope;
#[cfg(feature="flotalk")] pub mod flotalk;
