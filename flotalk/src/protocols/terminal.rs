use crate::*;

///
/// Protocol that represents a text terminal output stream
///
#[derive(Debug, TalkMessageType, PartialEq)]
pub enum TalkTerminalOut {
    // A TUI stream also acts as a puttable stream

    /// Writes a carriage return sequence to the stream
    #[message("cr")]
    Cr,

    /// Flushes the stream's backing store
    #[message("flush")]
    Flush,

    /// Writes a single character to the stream
    #[message("nextPut:")]
    NextPut(char),

    /// Writes all of the values in a collection to the stream
    #[message("nextPutAll:")]
    NextPutAll(TalkValue),

    /// Writes a space to the stream
    #[message("space")]
    Space,

    /// Writes a tab character to the stream
    #[message("tab")]
    Tab,

    // Crossterm commands

    /// Clears the entire state from the TUI buffer
    #[message("clearAll")]
    ClearAll,

    /// Writes a string to the stream
    #[message("say:")]
    Say(String),

    /// Turns off linewrap for the terminal
    #[message("disableLineWrap")]
    DisableLineWrap,

    /// Enables linewrapping for the terminal
    #[message("enableLineWrap")]
    EnableLineWrap,
}

///
/// Protocol representing a terminal input event
///
#[derive(Debug, TalkMessageType, PartialEq)]
pub enum TalkTerminalEvent {
    /// Sent to the stream when it's initialised with a true or false value to indicate if the terminal supports attributed text or not
    #[message("supportsAttributes:")]
    SupportsAttributes(bool),
}
