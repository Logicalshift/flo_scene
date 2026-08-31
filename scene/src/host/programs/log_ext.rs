use crate::host::scene_context::*;

use super::log::*;

use futures::prelude::*;

use std::borrow::{Cow};
use std::collections::*;
use std::sync::*;

/// Number of waiting log messages that indicates the log program is backed up
const LOG_BACKED_UP: usize = 50;

/// Number of waiting log messages that indicates the log is no longer processing messages properly
const LOG_FAILED: usize = 1000;

///
/// Convenience functions for interacting with `Log` directly
///
pub trait SceneLogExt {
    ///
    /// Sends a log message to the logging program
    ///
    fn log(&self, log_level: LogLevel, msg: impl Into<Cow<'static, str>>);

    ///
    /// Reports a message using the 'debug' logging level
    ///
    fn debug(&self, msg: impl Into<Cow<'static, str>>) {
        self.log(LogLevel::Debug, msg);
    }

    ///
    /// Reports a message using the 'info' logging level
    ///
    fn info(&self, msg: impl Into<Cow<'static, str>>) {
        self.log(LogLevel::Info, msg);
    }

    ///
    /// Reports a message using the 'warning' logging level
    ///
    fn warn(&self, msg: impl Into<Cow<'static, str>>) {
        self.log(LogLevel::Warn, msg);
    }

    ///
    /// Reports a message using the 'error' warning level
    ///
    fn error(&self, msg: impl Into<Cow<'static, str>>) {
        self.log(LogLevel::Error, msg);
    }

    ///
    /// Reports a message using the 'fatal' warning level
    ///
    fn fatal(&self, msg: impl Into<Cow<'static, str>>) {
        self.log(LogLevel::Fatal, msg);
    }
}

///
/// Log messages waiting to be delivered by the background task
///
struct LogQueue {
    waiting: VecDeque<Log>,
}

impl Default for LogQueue {
    fn default() -> Self {
        Self {
            waiting: VecDeque::new(),
        }
    }
}

impl SceneLogExt for SceneContext {
    fn log(&self, log_level: LogLevel, msg: impl Into<Cow<'static, str>>) {
        // Need the program ID to send as part of the log message
        let Some(our_program_id) = self.current_program_id() else { return; };

        let msg = match log_level {
            LogLevel::Debug => Log::Debug(our_program_id, msg.into()),
            LogLevel::Info  => Log::Info(our_program_id, msg.into()),
            LogLevel::Warn  => Log::Warn(our_program_id, msg.into()),
            LogLevel::Error => Log::Error(our_program_id, msg.into()),
            LogLevel::Fatal => Log::Fatal(our_program_id, msg.into()),
        };

        // We send log messages via a background process: if it's running we add to our existing queue, otherwise we start a new background process
        // Typically, the log has slots available so messages are delivered very fast and not queued
        let Some(queue) = self.get::<Arc<Mutex<LogQueue>>>() else { return; };

        let mut queue_lock = queue.lock().unwrap();

        if queue_lock.waiting.is_empty() {
            // No messages waiting: send immediately, starting a background process that will drain the queue if it's full
            queue_lock.waiting.push_back(msg);

            // Release the log to stop run_in_background deadlocking when it tries the fast path
            drop(queue_lock);

            // Start a background process (usually this will complete without actually polling)
            let Ok(mut log) = self.send(()) else { return; };

            self.run_in_background(async move {
                loop {
                    // Fetch the next message from the queue
                    let (next_msg, finished) = {
                        let mut queue_lock  = queue.lock().unwrap();
                        let next_msg        = queue_lock.waiting.pop_front().unwrap_or_else(|| Log::Error(our_program_id, "Log queue unexpectedly empty".into()));
                        let finished        = queue_lock.waiting.is_empty();

                        (next_msg, finished)
                    };

                    // Send the message on (queue can fill up if we're here)
                    log.send(next_msg).await.ok();

                    // Stop once the queue is entirely drained
                    if finished {
                        break;
                    }
                }
            });
        } else {
            // Push the message onto the queue, where it'll be picked up by the already-running background process
            queue_lock.waiting.push_back(msg);

            if queue_lock.waiting.len() >= LOG_FAILED {
                let num_waiting = queue_lock.waiting.len();
                drop(queue_lock);
                panic!("{:?}: logging subprogram has failed: {} messages waiting", our_program_id, num_waiting);
            }

            // If we pass a threshold, try immediate mode (will often block until the logger processes its backlog, so will prevent)
            if queue_lock.waiting.len() == LOG_BACKED_UP {
                drop(queue_lock);

                let Ok(mut log) = self.send(()) else { return; };
                log.send_immediate(Log::Error(our_program_id, format!("Reached {} waiting log messages (log subprogram is backed up)", LOG_BACKED_UP).into())).ok();
            }
        }
    }
}
