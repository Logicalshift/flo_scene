use super::puttable_stream::*;

use crate::*;

///
/// Protocol that represents a text terminal output stream
///
#[derive(Debug, PartialEq)]
pub enum TalkTerminalOut {
    /// Standard puttable stream request
    Put(TalkPuttableStreamRequest),

    /// Terminal command request
    Terminal(TalkTerminalCmd),
}

#[derive(Debug, TalkMessageType, PartialEq)]
pub enum TalkTerminalCmd {
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

    /// Displays the alternate screen
    #[message("enterAlternateScreen")]
    EnterAlternateScreen,

    /// Leaves the alternate screen
    #[message("leaveAlternateScreen")]
    LeaveAlternateScreen,

    /// Scrolls down
    #[message("scrollDown:")]
    ScrollDown(i32),

    /// Scrolls up
    #[message("scrollUp:")]
    ScrollUp(i32),

    /// Sets the size of the terminal buffer
    #[message("setSizeWidth:height:")]
    SetSize(i32, i32),

    /// Changes the title of the terminal window
    #[message("setTitle:")]
    SetTitle(String),
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

///
/// TalkTerminalOut just uses the messages from its 'subtypes' directly
///
impl TalkMessageType for TalkTerminalOut {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        TalkPuttableStreamRequest::supports_message(id)
            || TalkTerminalCmd::supports_message(id)
    }

    /// Converts a message to an object of this type
    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        use TalkTerminalOut::*;

        if TalkPuttableStreamRequest::supports_message(message.signature_id()) {
            
            Ok(Put(TalkPuttableStreamRequest::from_message(message, context)?))
        
        } else if TalkTerminalCmd::supports_message(message.signature_id()) {
            
            Ok(Terminal(TalkTerminalCmd::from_message(message.clone(), context)?))

        } else {
            Err(TalkError::MessageNotSupported(message.signature_id()))
        }
    }

    /// Converts an object of this type to a message
    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        use TalkTerminalOut::*;

        match self {
            Put(put)            => put.to_message(context),
            Terminal(terminal)  => terminal.to_message(context),
        }
    }
}
