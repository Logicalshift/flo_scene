//!
//! `flo_scene_pipe` provides ways to connect scenes created in `flo_scene` across process boundaries.
//!
//! This can create socket programs using different protocols. These will automatically accept connections
//! and pass them on as messages. For example, `start_unencrypted_tcp_socket()` will create a TCP/IP socket
//! subprogram that will decode and encode messages for the scene. Socket subprograms send 
//! `SocketMessage::Connection()` messages to the scene to indicate when a new connection is established.
//! Receivers of this message can use it to read and write messages to the corresponding connection.
//!
//! Internal (in-process), TCP and unix domain sockets are supported.
//!
//! This also adds an interactive command interpreter that can be used to interact directly with a scene.
//! This provides a lot of use cases - for example:
//!
//!  * Debug or query a running system
//!  * Run automatic tests on a system that's in production at a lower level than just clicking around the UI or prodding public API instances (for example as part of a commisioning process)
//!  * Configure or reconfigure a system without needing to restart it
//!  * Load modules or upgrade them in place, particularly when combined with `flo_scene_wasm`
//!
//! The command interface runs as a subprogram that interacts with a socket. Commands are themselves served by subprograms.
//! For example, you can set up a unix-domain socket like this:
//!
//! ```no_run
//! use flo_scene::*;
//! use flo_scene_pipe::*;
//! use flo_scene_pipe::commands::*;
//!
//! // Create a scene with some commands
//! let scene = Scene::default()
//!     .with_standard_json_commands();
//!
//! // Create a program that will receive connections from the socket and run commands
//! let command_program = SubProgramId::new();
//! scene.add_subprogram(command_program, |input, context| command_connection_program(input, context, ()), 0);
//!
//! // Set up a UNIX socket that will parse commands in the standard syntax using `read_command_data`
//! // and write the results to the output using `write_command_data`
//! let socket_program = SubProgramId::new();
//! start_unix_socket_program(&scene, socket_program, "./example_unix_socket", read_command_data, write_command_data).unwrap();
//!
//! // Connect the programs together so socket connects are processed by the command program
//! scene.connect_programs(socket_program, command_program, StreamId::with_message_type::<CommandProgramSocketMessage>()).unwrap();
//! ```
//!
//! Connect to this socket with `socat - UNIX-CONNECT:./example_unix_socket` - it will present an interactive prompt where you
//! can type `help` to get more instructions.
//!
//! This pattern can be used to make other kinds of server: replace `read_command_data` and `write_command_data` with your
//! own functions to set the format of the data sent over the socket and replace `command_connection_program` to define how
//! those messages are processed. The different socket types can be chosen by changing `start_unix_socket_program` to 
//! something else.
//!
//! New commands can be created using `CommandLauncher` which can be used to configure a subprogram to process them.
//! For example:
//!
//! ```
//! # use flo_scene::*;
//! # use flo_scene_pipe::*;
//! # use flo_scene_pipe::commands::*;
//! # 
//! # let scene = Scene::default()
//! #     .with_standard_json_commands();
//! # 
//! use flo_scene::commands::*;
//! 
//! let command_program = SubProgramId::called("my_crate::my_commands");
//! let json_launcher = CommandLauncher::json()
//!     .with_json_command("::test", |param: String, _context| async move {
//!         CommandResponse::Json(serde_json::Value::String(param))
//!     });
//! scene.add_subprogram(command_program, json_launcher.to_subprogram(), 1);
//! ```
//!

mod socket;
mod unix_socket;
mod internal_socket;
mod tcp_socket;
mod tokenizer;
mod parse_json;
mod json_parse_error;
mod stdio_connection;

pub mod parser;
pub mod commands;
pub mod standard_json_commands;

pub use socket::*;
pub use unix_socket::*;
pub use internal_socket::*;
pub use tcp_socket::*;
pub use stdio_connection::*;

pub use commands::{JsonCommandLauncherExt};
pub use standard_json_commands::{StandardCommandsLauncherExt, StandardCommandsSceneExt};
