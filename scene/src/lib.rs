//!
//! # flo_scene
//! 
//! flo_scene is a crate that provides a way to build large pieces of software by combining smaller
//! 'sub-programs' that communicate via messages. This simplifies dependencies over more traditional
//! object-oriented techniques, as sub-programs do not need to directly depend on each other. There
//! are also benefits in terms of testing, code re-use and configurability.
//! 
//! ## Basic usage
//! 
//! Scenes are created using `Scene::default()` or `Scene::empty()`. The empty scene contains no
//! subprograms by default but the default scene contains some default ones, in particular a
//! control program that can be used to start other programs or define connections between programs.
//! 
//! ```
//! use flo_scene::*;
//! 
//! let scene = Scene::default();
//! ```
//! 
//! Sub-programs read from a single input stream of messages and can write to any number of output
//! streams. Each output stream can be connected to the input of another program, and these connections
//! can be specified independently of the programs themselves. Messages need to implement the 
//! `SceneMessage` trait, and subprograms can be added to a scene using the `add_subprogram()` function.
//! 
//! ```
//! # use flo_scene::*;
//! # use futures::prelude::*;
//! # let scene = Scene::default();
//! #
//! // Simple logger
//! #[derive(Debug)]
//! pub enum LogMessage {
//!     Info(String),
//!     Warning(String),
//!     Error(String)
//! }
//! 
//! impl SceneMessage for LogMessage { }
//! 
//! let log_program = SubProgramId::called("Logger");
//! scene.add_subprogram(log_program,
//!     |mut log_messages: InputStream<LogMessage>, _context| async move {
//!         while let Some(log_message) = log_messages.next().await {
//!             match log_message {
//!                 LogMessage::Info(msg)       => { println!("INFO:    {:?}", msg); }
//!                 LogMessage::Warning(msg)    => { println!("WARNING: {:?}", msg); }
//!                 LogMessage::Error(msg)      => { println!("ERROR:   {:?}", msg); }
//!             }
//!         }
//!     },
//!     10)
//! ```
//! 
//! Connections can be defined by the `connect_programs()` function. Note how this means that something
//! that generates log messages does not need to know their destination and that it's possible to change
//! how something is logging at run-time if needed.
//! 
//! ```
//! # use flo_scene::*;
//! # use futures::prelude::*;
//! # let scene = Scene::default();
//! # let log_program = SubProgramId::new();
//! # pub enum LogMessage { Warning(String) };
//! # impl SceneMessage for LogMessage { }
//! # scene.add_subprogram(log_program, |_: InputStream<LogMessage>, _| async { }, 0);
//! #
//! // Connect any program that writes log messages to our log program
//! // '()' means 'any source' here, it's possible to define connections on a per-subprogram basis if needed.
//! scene.connect_programs((), log_program, StreamId::with_message_type::<LogMessage>()).unwrap();
//! ```
//! 
//! Subprograms have a context that can be used to retrieve output streams, or send single messages, so
//! after this connection is set up, anything can send log messages.
//! 
//! ```
//! # use flo_scene::*;
//! # use futures::prelude::*;
//! # let scene = Scene::default();
//! # #[derive(Debug)]
//! # pub enum LogMessage { Warning(String) };
//! # impl SceneMessage for LogMessage { }
//! #
//! let test_program = SubProgramId::new();
//! scene.add_subprogram(test_program,
//!     |_: InputStream<()>, context| async move {
//!         // '()' means send to any target
//!         let mut logger = context.send::<LogMessage>(()).unwrap();
//! 
//!         // Will send to the logger program
//!         logger.send(LogMessage::Warning("Hello".to_string())).await.unwrap();
//!     },
//!     0);
//! ```
//! 
//! Once set up, the scene needs to be run, in the async context of your choice:
//! 
//! ```Rust
//! executor::block_on(async {
//!     scene.run_scene().await;
//! });
//! ```
//! 
//! A single scene can be run in multiple threads if needed and subprograms are naturally able to run
//! asynchronously as they communicate with messages rather than by direct data access.
//! 
//! ## A few more advanced things
//! 
//! When the scene is created with `Scene::default()`, a control program is present that allows
//! subprograms to start other subprograms or create connections:
//! 
//! ```Rust
//!     /* ... */
//!     context.send_message(SceneControl::start_program(new_program_id, |input, context| /* ... */, 10)).await.unwrap();
//!     context.send_message(SceneControl::connect(some_program, some_other_program, StreamId::for_message_type::<MyMessage>())).await.unwrap();
//! ```
//! 
//! The empty scene does not get this control program (it's possible to use the `Scene` struct directly though).
//! 
//! Filters make it possible to connect two subprograms that take different message types by transforming
//! them. They need to be registered, then they can be used as a stream target:
//! 
//! ```Rust
//! // Note that the stream ID identifies the *source* stream: there's only one input stream for any program
//! let mine_to_yours_filter = FilterHandle::for_filter(|my_messages: InputStream<MyMessage>| my_messages.map(|msg| YourMessage::from(msg)));
//! scene.connect(my_program, StreamTarget::Filtered(mine_to_yours_filter, your_program), StreamId::with_message_type::<MyMessage>()).unwrap();
//! ```
//! 
//! The message type has some functions that can be overridden to provide some default behaviour which
//! can remove the need to manually configure connections whenever a scene is created:
//! 
//! ```Rust
//! impl SceneMessage for MyMessage {
//!     fn default_target() -> StreamTarget {
//!         StreamTarget::Program(SubProgramId::called("MyMessageHandler"))
//!     }
//! }
//! ```
//! 
//! A program can 'upgrade' its input stream to annotate the messages with their source if it needs this information:
//! 
//! ```Rust
//! scene.add_subprogram(log_program,
//!     |log_messages: InputStream<LogMessage>, _context| async move {
//!         let mut log_messages = log_messages.messages_with_sources();
//!         while let Some((source, log_message)) = log_messages.next().await {
//!             match log_message {
//!                 LogMessage::Info(msg)       => { println!("INFO:    [{:?}] {:?}", source, msg); }
//!                 LogMessage::Warning(msg)    => { println!("WARNING: [{:?}] {:?}", source, msg); }
//!                 LogMessage::Error(msg)      => { println!("ERROR:   [{:?}] {:?}", source, msg); }
//!             }
//!         }
//!     },
//!     10)
//! ```
//! 
//! It is possible to request a stream directly to a particular program:
//! 
//! ```Rust
//!     let mut logger = context.send::<LogMessage>(SubProgramId::called("MoreSpecificLogger"));
//! ```
//! 
//! But it's also possible to redirect these with a connection request:
//! 
//! ```Rust
//! // The scene is the ultimate arbiter of who can talk to who, so if we don't want our program talking to the MoreSpecificLogger after all we can change that
//! // Take care as this can get confusing!
//! scene.connect(exception_program, standard_logger_program, StreamId::with_message_type::<LogMessage>().for_target(SubProgramId::called("MoreSpecificLogger")));
//! ```
//! 
//! You can create and run more than one `Scene` at once if needed.
//! 

#![allow(clippy::redundant_field_names)]            // I prefer this to be consistent across the struct when initialising

mod host;

pub use host::*;
pub (crate) use host::scene;
pub (crate) use host::scene_core;
pub (crate) use host::subprogram_core;
pub (crate) use host::process_core;
pub (crate) use host::subprogram_id;
pub (crate) use host::stream_id;
pub (crate) use host::stream_source;
pub (crate) use host::stream_target;
pub (crate) use host::input_stream;
pub (crate) use host::output_sink;
pub (crate) use host::filter;
pub (crate) use host::scene_message;
pub (crate) use host::thread_stealer;
pub (crate) use host::command_trait;
pub (crate) use host::connect_result;
