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

static SYMBOL_RESET: Lazy<TalkMessageSignatureId>       = Lazy::new(|| "reset".into());
static SYMBOL_BLACK: Lazy<TalkMessageSignatureId>       = Lazy::new(|| "black".into());
static SYMBOL_DARKGREY: Lazy<TalkMessageSignatureId>    = Lazy::new(|| "darkGrey".into());
static SYMBOL_RED: Lazy<TalkMessageSignatureId>         = Lazy::new(|| "red".into());
static SYMBOL_DARKRED: Lazy<TalkMessageSignatureId>     = Lazy::new(|| "darkRed".into());
static SYMBOL_GREEN: Lazy<TalkMessageSignatureId>       = Lazy::new(|| "green".into());
static SYMBOL_DARKGREEN: Lazy<TalkMessageSignatureId>   = Lazy::new(|| "darkGreen".into());
static SYMBOL_YELLOW: Lazy<TalkMessageSignatureId>      = Lazy::new(|| "yellow".into());
static SYMBOL_DARKYELLOW: Lazy<TalkMessageSignatureId>  = Lazy::new(|| "darkYellow".into());
static SYMBOL_BLUE: Lazy<TalkMessageSignatureId>        = Lazy::new(|| "blue".into());
static SYMBOL_DARKBLUE: Lazy<TalkMessageSignatureId>    = Lazy::new(|| "darkBlue".into());
static SYMBOL_MAGENTA: Lazy<TalkMessageSignatureId>     = Lazy::new(|| "magenta".into());
static SYMBOL_DARKMAGENTA: Lazy<TalkMessageSignatureId> = Lazy::new(|| "darkMagenta".into());
static SYMBOL_CYAN: Lazy<TalkMessageSignatureId>        = Lazy::new(|| "cyan".into());
static SYMBOL_DARKCYAN: Lazy<TalkMessageSignatureId>    = Lazy::new(|| "darkCyan".into());
static SYMBOL_WHITE: Lazy<TalkMessageSignatureId>       = Lazy::new(|| "white".into());
static SYMBOL_GREY: Lazy<TalkMessageSignatureId>        = Lazy::new(|| "grey".into());
static MSG_RGB: Lazy<TalkMessageSignatureId>            = Lazy::new(|| ("r:", "g:", "b:").into());

impl TalkValueType for TalkStyleColor {
    fn into_talk_value<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkValue, &'a TalkContext> {
        use TalkStyleColor::*;

        let value = match self {
            Reset           => TalkValue::Selector(*SYMBOL_RESET),
            Black           => TalkValue::Selector(*SYMBOL_BLACK),
            DarkGrey        => TalkValue::Selector(*SYMBOL_DARKGREY),
            Red             => TalkValue::Selector(*SYMBOL_RED),
            DarkRed         => TalkValue::Selector(*SYMBOL_DARKRED),
            Green           => TalkValue::Selector(*SYMBOL_GREEN),
            DarkGreen       => TalkValue::Selector(*SYMBOL_DARKGREEN),
            Yellow          => TalkValue::Selector(*SYMBOL_YELLOW),
            DarkYellow      => TalkValue::Selector(*SYMBOL_DARKYELLOW),
            Blue            => TalkValue::Selector(*SYMBOL_BLUE),
            DarkBlue        => TalkValue::Selector(*SYMBOL_DARKBLUE),
            Magenta         => TalkValue::Selector(*SYMBOL_MAGENTA),
            DarkMagenta     => TalkValue::Selector(*SYMBOL_DARKMAGENTA),
            Cyan            => TalkValue::Selector(*SYMBOL_CYAN),
            DarkCyan        => TalkValue::Selector(*SYMBOL_DARKCYAN),
            White           => TalkValue::Selector(*SYMBOL_WHITE),
            Grey            => TalkValue::Selector(*SYMBOL_GREY),
            Rgb { r, g, b } => TalkValue::Message(Box::new(TalkMessage::from_signature(*MSG_RGB, smallvec![(*r).into(), (*g).into(), (*b).into()])))
        };

        TalkOwned::new(value, context)
    }

    fn try_from_talk_value<'a>(value: TalkOwned<TalkValue, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        use TalkStyleColor::*;

        match &*value {
            TalkValue::Selector(msg_id) => {
                if *msg_id == *SYMBOL_RESET             { Ok(Reset) }
                else if *msg_id == *SYMBOL_BLACK        { Ok(Black) }
                else if *msg_id == *SYMBOL_DARKGREY     { Ok(DarkGrey) }
                else if *msg_id == *SYMBOL_RED          { Ok(Red) }
                else if *msg_id == *SYMBOL_DARKRED      { Ok(DarkRed) }
                else if *msg_id == *SYMBOL_GREEN        { Ok(Green) }
                else if *msg_id == *SYMBOL_DARKGREEN    { Ok(DarkGreen) }
                else if *msg_id == *SYMBOL_YELLOW       { Ok(Yellow) }
                else if *msg_id == *SYMBOL_DARKYELLOW   { Ok(DarkYellow) }
                else if *msg_id == *SYMBOL_BLUE         { Ok(Blue) }
                else if *msg_id == *SYMBOL_DARKBLUE     { Ok(DarkBlue) }
                else if *msg_id == *SYMBOL_MAGENTA      { Ok(Magenta) }
                else if *msg_id == *SYMBOL_DARKMAGENTA  { Ok(DarkMagenta) }
                else if *msg_id == *SYMBOL_CYAN         { Ok(Cyan) }
                else if *msg_id == *SYMBOL_DARKCYAN     { Ok(DarkCyan) }
                else if *msg_id == *SYMBOL_WHITE        { Ok(White) }
                else if *msg_id == *SYMBOL_GREY         { Ok(Grey) }
                else                                    { Err(TalkError::UnexpectedSelector(*msg_id))}
            }

            TalkValue::Message(msg) => {
                if msg.signature_id() == *MSG_RGB {
                    let args    = msg.arguments().unwrap();
                    let r       = i32::try_from_talk_value(TalkOwned::new(args[0].clone_in_context(context), context), context)?;
                    let g       = i32::try_from_talk_value(TalkOwned::new(args[1].clone_in_context(context), context), context)?;
                    let b       = i32::try_from_talk_value(TalkOwned::new(args[2].clone_in_context(context), context), context)?;

                    Ok(Rgb { r, g, b })
                } else {
                    Err(TalkError::UnexpectedSelector(msg.signature_id()))
                }
            }

            _ => Err(TalkError::NotASelector),
        }
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
