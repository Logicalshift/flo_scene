use super::puttable_stream::*;

use crate::*;

use smallvec::*;
use once_cell::sync::{Lazy};

///
/// Protocol that represents a text terminal output stream
///
#[derive(Debug, PartialEq)]
pub enum TalkTerminalOut {
    /// Standard puttable stream request
    Put(TalkPuttableStreamRequest),

    /// Terminal command request
    Terminal(TalkTerminalCmd),

    /// Styling command
    Style(TalkTextStyleCmd),
}

///
/// Commands that relate to controlling a terminal display
///
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
/// Commands that related to styling text
///
#[derive(Debug, TalkMessageType, PartialEq)]
pub enum TalkTextStyleCmd {
    #[message("styleAttribute:")]
    SetAttribute(TalkStyleAttribute),

    #[message("foregroundColor:")]
    SetForeground(TalkStyleColor),

    #[message("backgroundColor:")]
    SetBackground(TalkStyleColor),

    #[message("forgroundColor:backgroundColor:")]
    SetColors(TalkStyleColor, TalkStyleColor),

    #[message("underlineColor:")]
    SetUnderlineColor(TalkStyleColor),
}

///
/// Colours supported by the terminal
///
/// (These match those in crossterm, but they support flotalk's conversion interfaces, they're passed in as selectors except for Rgb which needs to be a message)
///
#[derive(Debug, PartialEq)]
pub enum TalkStyleColor {
    Reset,
    Black,
    DarkGrey,
    Red,
    DarkRed,
    Green,
    DarkGreen,
    Yellow,
    DarkYellow,
    Blue,
    DarkBlue,
    Magenta,
    DarkMagenta,
    Cyan,
    DarkCyan,
    White,
    Grey,
    Rgb {
        r: i32,
        g: i32,
        b: i32,
    },
}

///
/// Attributes supported by the terminal
///
/// (These match those in crossterm, but they support flotalk's conversion interfaces, they're passed in as selectors)
///
#[derive(Debug, PartialEq)]
pub enum TalkStyleAttribute {
    Reset,
    Bold,
    Dim,
    Italic,
    Underlined,
    DoubleUnderlined,
    Undercurled,
    Underdotted,
    Underdashed,
    SlowBlink,
    RapidBlink,
    Reverse,
    Hidden,
    CrossedOut,
    Fraktur,
    NoBold,
    NormalIntensity,
    NoItalic,
    NoUnderline,
    NoBlink,
    NoReverse,
    NoHidden,
    NotCrossedOut,
    Framed,
    Encircled,
    OverLined,
    NotFramedOrEncircled,
    NotOverLined,
}

///
/// Protocol representing a terminal input event
///
#[derive(Debug, TalkMessageType, PartialEq)]
pub enum TalkTerminalEvent {
    /// Sent to the stream when it's initialised with a true or false value to indicate if the terminal supports terminal commands or not. If this is false, then
    /// only the 'puttableStream' instructions will do anything
    #[message("SupportsTerminalCommands:")]
    SupportsTerminalCommands(bool),
}

impl TalkValueType for TalkStyleColor {
    fn into_talk_value<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkValue, &'a TalkContext> {
        use TalkStyleColor::*;

        static SYMBOL_RESET: Lazy<TalkValue>         = Lazy::new(|| TalkValue::Selector("reset".into()));
        static SYMBOL_BLACK: Lazy<TalkValue>         = Lazy::new(|| TalkValue::Selector("black".into()));
        static SYMBOL_DARKGREY: Lazy<TalkValue>      = Lazy::new(|| TalkValue::Selector("darkGrey".into()));
        static SYMBOL_RED: Lazy<TalkValue>           = Lazy::new(|| TalkValue::Selector("red".into()));
        static SYMBOL_DARKRED: Lazy<TalkValue>       = Lazy::new(|| TalkValue::Selector("darkRed".into()));
        static SYMBOL_GREEN: Lazy<TalkValue>         = Lazy::new(|| TalkValue::Selector("green".into()));
        static SYMBOL_DARKGREEN: Lazy<TalkValue>     = Lazy::new(|| TalkValue::Selector("darkGreen".into()));
        static SYMBOL_YELLOW: Lazy<TalkValue>        = Lazy::new(|| TalkValue::Selector("yellow".into()));
        static SYMBOL_DARKYELLOW: Lazy<TalkValue>    = Lazy::new(|| TalkValue::Selector("darkYellow".into()));
        static SYMBOL_BLUE: Lazy<TalkValue>          = Lazy::new(|| TalkValue::Selector("blue".into()));
        static SYMBOL_DARKBLUE: Lazy<TalkValue>      = Lazy::new(|| TalkValue::Selector("darkBlue".into()));
        static SYMBOL_MAGENTA: Lazy<TalkValue>       = Lazy::new(|| TalkValue::Selector("magenta".into()));
        static SYMBOL_DARKMAGENTA: Lazy<TalkValue>   = Lazy::new(|| TalkValue::Selector("darkMagenta".into()));
        static SYMBOL_CYAN: Lazy<TalkValue>          = Lazy::new(|| TalkValue::Selector("cyan".into()));
        static SYMBOL_DARKCYAN: Lazy<TalkValue>      = Lazy::new(|| TalkValue::Selector("darkCyan".into()));
        static SYMBOL_WHITE: Lazy<TalkValue>         = Lazy::new(|| TalkValue::Selector("white".into()));
        static SYMBOL_GREY: Lazy<TalkValue>          = Lazy::new(|| TalkValue::Selector("grey".into()));
        static MSG_RGB: Lazy<TalkMessageSignatureId> = Lazy::new(|| ("r:", "g:", "b:").into());

        let value = match self {
            Reset           => (*SYMBOL_RESET).clone(),
            Black           => (*SYMBOL_BLACK).clone(),
            DarkGrey        => (*SYMBOL_DARKGREY).clone(),
            Red             => (*SYMBOL_RED).clone(),
            DarkRed         => (*SYMBOL_DARKRED).clone(),
            Green           => (*SYMBOL_GREEN).clone(),
            DarkGreen       => (*SYMBOL_DARKGREEN).clone(),
            Yellow          => (*SYMBOL_YELLOW).clone(),
            DarkYellow      => (*SYMBOL_DARKYELLOW).clone(),
            Blue            => (*SYMBOL_BLUE).clone(),
            DarkBlue        => (*SYMBOL_DARKBLUE).clone(),
            Magenta         => (*SYMBOL_MAGENTA).clone(),
            DarkMagenta     => (*SYMBOL_DARKMAGENTA).clone(),
            Cyan            => (*SYMBOL_CYAN).clone(),
            DarkCyan        => (*SYMBOL_DARKCYAN).clone(),
            White           => (*SYMBOL_WHITE).clone(),
            Grey            => (*SYMBOL_GREY).clone(),
            Rgb { r, g, b } => TalkValue::Message(Box::new(TalkMessage::from_signature(*MSG_RGB, smallvec![(*r).into(), (*g).into(), (*b).into()])))
        };

        TalkOwned::new(value, context)
    }

    fn try_from_talk_value<'a>(value: TalkOwned<TalkValue, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        todo!()
    }
}

impl TalkValueType for TalkStyleAttribute {
    fn into_talk_value<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkValue, &'a TalkContext> {
        use TalkStyleAttribute::*;

        todo!()
    }

    fn try_from_talk_value<'a>(value: TalkOwned<TalkValue, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        todo!()
    }
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

        } else if TalkTextStyleCmd::supports_message(message.signature_id()) {

            Ok(Style(TalkTextStyleCmd::from_message(message.clone(), context)?))

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
            Style(style)        => style.to_message(context),
        }
    }
}
