use crate::host::filter::*;
use crate::host::input_stream::*;
use crate::host::initialisation_context::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;
use crate::host::commands::*;

use super::control::*;
use super::control_ext::*;
use super::query::*;
use super::subscription::*;
use super::text_output::*;

use futures::prelude::*;
use serde::*;

use std::borrow::{Cow};
use std::collections::*;

/// The identifier for the standard scene log program
pub static LOG_PROGRAM: StaticSubProgramId = StaticSubProgramId::called("flo_scene::log");

///
/// Log is a subprogram that generates or records messages relating to the operation
/// of a scene. By default it writes to the standard error stream, but if there are
/// any subscriptions it will only send 
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Log {
    /// Logs a debugging message
    Debug(SubProgramId, Cow<'static, str>),

    /// Logs an informational message
    Info(SubProgramId, Cow<'static, str>),

    /// Logs a warning message
    Warn(SubProgramId, Cow<'static, str>),

    /// Logs an error message
    Error(SubProgramId, Cow<'static, str>),

    /// Logs a fatal error message
    Fatal(SubProgramId, Cow<'static, str>),
}

///
/// The severity level of a log message
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Fatal
}

/// The log levels in order
const LOG_LEVELS: [LogLevel; 5] = [ LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error, LogLevel::Fatal ];

///
/// Message sent to a logger to subscribe to log messages
///
/// When there is at least one subscriber, the logger does not output any messages to
/// stderr. If no subscribers are present, log messages are sent to stderr by default.
///
/// (Redirecting the log messages somewhere else is another way to completely disable
/// any output to stderr, in case the subscription cannot be established fast enough)
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogSubscription {
    Subscribe(LogLevel, StreamTarget)
}

///
/// Message sent to the default log program
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub (crate) enum LogOrSubscription {
    Log(Log),
    Subscription(LogSubscription),
}

impl SceneMessage for Log {
    fn message_type_name() -> String {
        "Log".into()
    }

    fn default_target() -> StreamTarget {
        (*LOG_PROGRAM).into()
    }

    fn initialise(scene: &impl SceneInitialisationContext) {
        scene.connect_programs(
            (), 
            StreamTarget::Filtered(FilterHandle::for_filter(|msgs| 
                msgs.map(|msg| LogOrSubscription::Log(msg))), *LOG_PROGRAM), 
            StreamId::with_message_type::<Log>()
        ).unwrap();
    }
}

impl SceneMessage for LogSubscription {
    fn message_type_name() -> String {
        "LogSubscription".into()
    }

    fn default_target() -> StreamTarget {
        (*LOG_PROGRAM).into()
    }

    fn initialise(scene: &impl SceneInitialisationContext) {
        scene.connect_programs(
            (), 
            StreamTarget::Filtered(FilterHandle::for_filter(|msgs: InputStream<Subscribe<Log>>| 
                msgs.map(|msg| LogOrSubscription::Subscription(LogSubscription::Subscribe(LogLevel::Debug, msg.target())))), *LOG_PROGRAM), 
            StreamId::with_message_type::<Subscribe<Log>>()
        ).unwrap();

        scene.connect_programs(
            (), 
            StreamTarget::Filtered(FilterHandle::for_filter(|msgs| 
                msgs.map(|msg| LogOrSubscription::Subscription(msg))), *LOG_PROGRAM), 
            StreamId::with_message_type::<LogSubscription>()
        ).unwrap();
    }
}

impl SceneMessage for LogOrSubscription {
    fn message_type_name() -> String {
        "LogOrSubscription".into()
    }
}

impl Log {
    ///
    /// The log level of this message
    ///
    pub fn level(&self) -> LogLevel {
        match self {
            Log::Debug(_, _)    => LogLevel::Debug,
            Log::Info(_, _)     => LogLevel::Info,
            Log::Warn(_, _)     => LogLevel::Warn,
            Log::Error(_, _)    => LogLevel::Error,
            Log::Fatal(_, _)    => LogLevel::Fatal,
        }
    }

    ///
    /// The source of this message
    ///
    pub fn source(&self) -> SubProgramId {
        match self {
            Log::Debug(program_id, _)   |
            Log::Info(program_id, _)    |
            Log::Warn(program_id, _)    |
            Log::Error(program_id, _)   |
            Log::Fatal(program_id, _)   => *program_id
        }
    }

    ///
    /// Formats a log string to a particular width, optionally using ANSI control codes
    ///
    pub fn format_log_string(&self, program_names: &HashMap<SubProgramId, Option<Cow<'static, str>>>, width: usize, use_ansi_codes: bool) -> String {
        // Break this message down
        let (level, source, message) = match self {
            Log::Debug(source, message)     => (LogLevel::Debug, source, message),
            Log::Info(source, message)      => (LogLevel::Info, source, message),
            Log::Warn(source, message)      => (LogLevel::Warn, source, message),
            Log::Error(source, message)     => (LogLevel::Error, source, message),
            Log::Fatal(source, message)     => (LogLevel::Fatal, source, message),
        };

        // Figure out how much space to give the program name and the log message
        let max_program_name_len    = width / 3;
        let program_name_len        = 38.min(max_program_name_len);

        let program_name = if let Some(Some(name)) = program_names.get(source) {
            name.clone()
        } else {
            source.to_string().into()
        };

        // Create the portions of the formatted message
        let truncated_name = if program_name.len() > program_name_len-2 {
            program_name.chars().take(program_name_len-2).collect()
        } else {
            let padding = (program_name_len-2) - program_name.len();
            format!("{}{}", program_name, (0..padding).map(|_| ' ').collect::<String>())
        };

        // Wrap the message to the remaining width
        let message_len         = (width - program_name_len) - 3;
        let mut wrapped_message = vec![];
        let mut current_line    = String::new();

        for c in message.chars() {
            // Wrap the current line when it's too long
            // TODO: word wrap might be better
            if current_line.len() >= message_len {
                wrapped_message.push(current_line);
                current_line = String::new();
            }

            if c >= ' ' {
                current_line.push(c);
            } else {
                current_line.push('.');
            }
        }

        wrapped_message.push(current_line);

        // Generate the result
        let formatted_name = if use_ansi_codes {
            let (code, uncode) = match level {
                LogLevel::Debug => ("\x1b[0;36m  ", "\x1b[0m"),
                LogLevel::Info  => ("\x1b[0;97mI ", "\x1b[0m"),
                LogLevel::Warn  => ("\x1b[0;33mW ", "\x1b[0m"),
                LogLevel::Error => ("\x1b[0;31m! ", "\x1b[0m"),
                LogLevel::Fatal => ("\x1b[1;91m!!", "\x1b[0m"),
            };

            format!("{}{}{}", code, truncated_name, uncode)
        } else {
            let code = match level {
                LogLevel::Debug => "  ",
                LogLevel::Info  => "I ",
                LogLevel::Warn  => "W ",
                LogLevel::Error => "! ",
                LogLevel::Fatal => "!!",
            };

            format!("{}{}", code, truncated_name)
        };

        if wrapped_message.len() == 1 {
            format!("{} | {}", formatted_name, wrapped_message[0])
        } else {
            use std::iter;

            let divider = (0..(program_name_len-1))
                .map(|_| '-')
                .chain(iter::once('+'))
                .chain((0..message_len).map(|_| '-'))
                .collect::<String>();

            let mut message = divider.clone();
            for (idx, msg) in wrapped_message.into_iter().enumerate() {
                if idx == 0 {
                    message.extend(format!("\n{} | {}", formatted_name, msg).chars());
                } else {
                    message.extend(format!("\n{} | {}", (0..(program_name_len-2)).map(|_| ' ').collect::<String>(), msg).chars());
                }
            }

            message
        }
    }

    ///
    /// Subprogram that writes log messages to stderr
    ///
    pub async fn stderr_log_output_program(input: InputStream<Log>, context: SceneContext) {
        context.i_am("Logging to stderr");

        let Ok(mut stderr) = context.send(()) else { return; };

        // Keep a list of program names (we query these when they're not known)
        let mut program_names = HashMap::new();

        let mut input = input;

        // Read logging messages
        while let Some(input) = input.next().await {
            // Attempt to discover the program names, if this program hasn't been seen before
            if !program_names.contains_key(&input.source()) {
                // Regenerate from scratch (removing any names that are no longer in the scene)
                program_names = HashMap::new();

                // Ensure the source name is present
                program_names.insert(input.source(), None);

                // Query the names from the control program
                let mut current_scene_state = context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), StreamTarget::Any).unwrap();
                while let Some(evt) = current_scene_state.next().await {
                    match evt {
                        SceneUpdate::Tagged(program_id, SceneProgramTag::Name(program_name)) => {
                            program_names.insert(program_id, Some(program_name.into()));
                        }

                        _ => { }
                    }
                }
            }

            // Write to stderr
            stderr.send(ErrorOutput::Line(format!("{}\n", input.format_log_string(&program_names, 80, true)).into())).await.ok();
        }
    }

    ///
    /// The default log subprogram
    ///
    pub (crate) async fn default_log_program(input: InputStream<LogOrSubscription>, context: SceneContext) {
        context.i_am("Logging");
        context.tag(SceneProgramTag::Namespace("flo_scene".into())).ok();

        // Start the stderr logging program as a child program
        let stderr_logger       = SubProgramId::new();
        let mut stderr_logger   = if context.add_child_subprogram(stderr_logger, Self::stderr_log_output_program, 100).is_ok() {
            context.send(stderr_logger).ok()
        } else {
            None
        };

        // Track the subscribers to this log program
        let mut subscribers = HashMap::new();

        // Process the input
        let mut input = input;

        while let Some(input) = input.next().await {
            match input {
                LogOrSubscription::Subscription(LogSubscription::Subscribe(level, target)) => {
                    subscribers.entry(level)
                        .or_insert_with(|| EventSubscribers::<Log>::new())
                        .subscribe(&context, target);
                }

                LogOrSubscription::Log(log_msg) => {
                    // If the log message is sent to the subscribers then we don't forward it to stderr (otherwise we do)
                    let mut sent_to_subscribers = false;
                    let msg_level               = log_msg.level();

                    for level in LOG_LEVELS.iter().copied() {
                        // If this logger should receive messages at this level, try sending to the subscribers (and remember if we did so)
                        if level <= msg_level {
                            if let Some(subscribers) = subscribers.get_mut(&level) {
                                sent_to_subscribers = subscribers.send(log_msg.clone()).await || sent_to_subscribers;
                            }
                        }
                    }

                    // If there were no subscribers for this log message, send to the stderr logger
                    if !sent_to_subscribers {
                        if let Some(stderr_logger) = stderr_logger.as_mut() {
                            stderr_logger.send(log_msg).await.ok();
                        }
                    }
                }
            }
        }
    }
}
