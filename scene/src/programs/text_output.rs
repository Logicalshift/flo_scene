use crate::*;

use futures::prelude::*;
use futures::{pin_mut};
use once_cell::sync::{Lazy};

use std::io::*;

pub static STDOUT_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("STDOUT_PROGRAM"));
pub static STDERR_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("STDERR_PROGRAM"));
static ERROR_TO_TEXT_FILTER: Lazy<FilterHandle> = Lazy::new(|| FilterHandle::for_filter(|stream: InputStream<ErrorOutput>| stream.map(|err| TextOutput::from(err))));

///
/// Messages for writing text to an output stream
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum TextOutput {
    /// Writes a single character to the output
    Character(char),

    /// Writes a string to the output
    Text(String),

    /// Writes some text at the start of a new line
    Line(String),
}

///
/// Messages for writing text to an error stream
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ErrorOutput {
    /// Writes a single character to the output
    Character(char),

    /// Writes a string to the output
    Text(String),

    /// Writes some text at the start of a new line
    Line(String),
}

impl From<ErrorOutput> for TextOutput {
    fn from(error_output: ErrorOutput) -> TextOutput {
        match error_output {
            ErrorOutput::Character(chr)     => TextOutput::Character(chr),
            ErrorOutput::Text(txt)          => TextOutput::Text(txt),
            ErrorOutput::Line(line)         => TextOutput::Line(line),
        }
    }
}

impl SceneMessage for TextOutput {
    fn default_target() -> StreamTarget             { (*STDOUT_PROGRAM).into() }
    fn allow_thread_stealing_by_default() -> bool   { true }
}

impl SceneMessage for ErrorOutput {
    fn default_target() -> StreamTarget             { (*STDERR_PROGRAM).into() }
    fn allow_thread_stealing_by_default() -> bool   { true }

    fn initialise(scene: &Scene) {
        // Convert ErrorOutput into TextOutput when sending to STDERR
        scene.connect_programs((), StreamTarget::Filtered(*ERROR_TO_TEXT_FILTER, *STDERR_PROGRAM), StreamId::with_message_type::<ErrorOutput>()).ok();
    }
}

///
/// The runtime for a text output subprogram, which will write messages to the specified target stream
///
pub async fn text_io_subprogram(target: impl Send + Write, messages: impl Stream<Item=TextOutput>, _: SceneContext) {
    pin_mut!(messages);

    let mut target              = target;
    let mut at_start_of_line    = false;

    while let Some(output) = messages.next().await {
        use TextOutput::*;

        match output {
            Character(chr)  => { write!(target, "{}", chr).ok(); at_start_of_line = chr == '\n'; },
            Text(text)      => { write!(target, "{}", text).ok(); at_start_of_line = text.ends_with('\n'); },
            Line(text)      => {
                if at_start_of_line {
                    write!(target, "{}", text).ok();
                } else {
                    write!(target, "\n{}", text).ok();
                }

                at_start_of_line = text.ends_with('\n');
            },
        }

        target.flush().ok();
    }
}
