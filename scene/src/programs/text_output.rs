use crate::*;

use futures::prelude::*;
use futures::{pin_mut};
use once_cell::sync::{Lazy};

use std::io::*;

pub static STDOUT_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("STDOUT_PROGRAM"));
pub static STDERR_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("STDERR_PROGRAM"));

///
/// Messages for writing text to an output stream
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum TextOutput {
    /// Writes a single character to the output
    Character(char),

    /// Writes a string to the output
    Text(String),

    /// Writes a string on its own line
    Line(String),
}

impl SceneMessage for TextOutput {
    fn default_target() -> StreamTarget             { (*STDOUT_PROGRAM).into() }
    fn allow_thread_stealing_by_default() -> bool   { true }
}

///
/// The runtime for a text output subprogram, which will write messages to the specified target stream
///
pub async fn text_io_subprogram(target: impl Send + Write, messages: impl Stream<Item=TextOutput>, _: SceneContext) {
    pin_mut!(messages);

    let mut target              = target;
    let mut messages            = messages;
    let mut at_start_of_line    = false;

    while let Some(output) = messages.next().await {
        use TextOutput::*;

        match output {
            Character(chr)  => { write!(target, "{}", chr).ok(); at_start_of_line = chr == '\n'; },
            Text(text)      => { write!(target, "{}", text).ok(); at_start_of_line = text.chars().last() == Some('\n'); },
            Line(text)      => {
                if at_start_of_line {
                    write!(target, "{}\n", text).ok();
                } else {
                    write!(target, "\n{}\n", text).ok();
                }

                at_start_of_line = true;
            },
        }
    }
}
