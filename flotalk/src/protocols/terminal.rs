use super::puttable_stream::*;

use crate::*;
use crate::standard_classes::*;

use futures::prelude::*;
use futures::{pin_mut};

use smallvec::*;
use once_cell::sync::{Lazy};

#[cfg(feature="crossterm")]
use crossterm;

use std::result::{Result};
use std::io::{Write};

///
/// Protocol that represents a text terminal output stream
///
#[derive(Debug, PartialEq)]
pub enum TalkTerminalOut {
    /// Standard puttable stream request
    Put(TalkSimpleStreamRequest),

    /// Terminal command request
    Terminal(TalkTerminalCmd),

    /// Styling command
    Style(TalkTextStyleCmd),

    /// Cursor command
    Cursor(TalkCursorCmd),
}

///
/// Commands that relate to controlling a terminal display
///
#[derive(Debug, TalkMessageType, PartialEq)]
pub enum TalkTerminalCmd {
    /// Clears the entire state from the TUI buffer
    #[message("clearAll")]
    ClearAll,

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
/// Commands related to styling text
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
/// Commands related to changing the terminal cursor
///
#[derive(Debug, TalkMessageType, PartialEq)]
pub enum TalkCursorCmd {
    /// Stops the cursor from blinking
    #[message("disableCursorBlink")]
    DisableBlinking,

    /// Enables cursor blinking
    #[message("enableCursorBlink")]
    EnableBlinking,

    /// Hides the cursor
    #[message("hideCursor")]
    Hide,

    /// Displays the cursor
    #[message("showCursor")]
    Show,

    /// Move the cursor down a number of rows
    #[message("moveCursorDown:")]
    MoveDown(i32),

    /// Move the cursor up a number of rows
    #[message("moveCursorUp:")]
    MoveUp(i32),

    /// Move the cursor left a number of rows
    #[message("moveCursorLeft:")]
    MoveLeft(i32),

    /// Move the cursor right a number of rows
    #[message("moveCursorRight:")]
    MoveRight(i32),

    /// Move the cursor to a specific position
    #[message("moveCursorToX:Y:")]
    MoveTo(i32, i32),

    /// Moves the cursor to a particular column
    #[message("moveCursorToX:")]
    MoveToColumn(i32),

    /// Moves the cursor to a particular row
    #[message("moveCursorToY:")]
    MoveToRow(i32),

    /// Moves the cursor down a number of lines, then to the start of the row
    #[message("moveCursorToNextLine:")]
    MoveToNextLine(i32),

    /// Moves the cursor up a number of lines, then to the start of the row
    #[message("moveCursorToPreviousLine:")]
    MoveToPreviousLine(i32),

    /// Puts the cursor back to the position it was in when SavePosition was called
    #[message("restoreCursorPosition")]
    RestorePosition,

    /// Stores the current position of the cursor
    #[message("saveCursorPosition")]
    SavePosition,
}

///
/// Colours supported by the terminal
///
/// (These match those in crossterm, but they support flotalk's conversion interfaces, they're passed in as selectors except for Rgb which needs to be a message)
///
#[derive(Copy, Clone, Debug, PartialEq)]
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
#[derive(Copy, Clone, Debug, PartialEq)]
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

static SYMBOL_BOLD: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| "bold".into());
static SYMBOL_DIM: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| "dim".into());
static SYMBOL_ITALIC: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "italic".into());
static SYMBOL_UNDERLINED: Lazy<TalkMessageSignatureId>              = Lazy::new(|| "underlined".into());
static SYMBOL_DOUBLEUNDERLINED: Lazy<TalkMessageSignatureId>        = Lazy::new(|| "doubleUnderlined".into());
static SYMBOL_UNDERCURLED: Lazy<TalkMessageSignatureId>             = Lazy::new(|| "undercurled".into());
static SYMBOL_UNDERDOTTED: Lazy<TalkMessageSignatureId>             = Lazy::new(|| "underdotted".into());
static SYMBOL_UNDERDASHED: Lazy<TalkMessageSignatureId>             = Lazy::new(|| "underdashed".into());
static SYMBOL_SLOWBLINK: Lazy<TalkMessageSignatureId>               = Lazy::new(|| "slowBlink".into());
static SYMBOL_RAPIDBLINK: Lazy<TalkMessageSignatureId>              = Lazy::new(|| "rapidBlink".into());
static SYMBOL_REVERSE: Lazy<TalkMessageSignatureId>                 = Lazy::new(|| "reverse".into());
static SYMBOL_HIDDEN: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "hidden".into());
static SYMBOL_CROSSEDOUT: Lazy<TalkMessageSignatureId>              = Lazy::new(|| "crossedOut".into());
static SYMBOL_FRAKTUR: Lazy<TalkMessageSignatureId>                 = Lazy::new(|| "fraktur".into());
static SYMBOL_NOBOLD: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "noBold".into());
static SYMBOL_NORMALINTENSITY: Lazy<TalkMessageSignatureId>         = Lazy::new(|| "normalIntensity".into());
static SYMBOL_NOITALIC: Lazy<TalkMessageSignatureId>                = Lazy::new(|| "noItalic".into());
static SYMBOL_NOUNDERLINE: Lazy<TalkMessageSignatureId>             = Lazy::new(|| "noUnderline".into());
static SYMBOL_NOBLINK: Lazy<TalkMessageSignatureId>                 = Lazy::new(|| "noBlink".into());
static SYMBOL_NOREVERSE: Lazy<TalkMessageSignatureId>               = Lazy::new(|| "noReverse".into());
static SYMBOL_NOHIDDEN: Lazy<TalkMessageSignatureId>                = Lazy::new(|| "noHidden".into());
static SYMBOL_NOTCROSSEDOUT: Lazy<TalkMessageSignatureId>           = Lazy::new(|| "notCrossedOut".into());
static SYMBOL_FRAMED: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "framed".into());
static SYMBOL_ENCIRCLED: Lazy<TalkMessageSignatureId>               = Lazy::new(|| "encircled".into());
static SYMBOL_OVERLINED: Lazy<TalkMessageSignatureId>               = Lazy::new(|| "overLined".into());
static SYMBOL_NOTFRAMEDORENCIRCLED: Lazy<TalkMessageSignatureId>    = Lazy::new(|| "notFramedOrEncircled".into());
static SYMBOL_NOTOVERLINED: Lazy<TalkMessageSignatureId>            = Lazy::new(|| "notOverLined".into());

impl TalkValueType for TalkStyleAttribute {
    fn into_talk_value<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkValue, &'a TalkContext> {
        use TalkStyleAttribute::*;

        let value = match self {
            Reset                   => TalkValue::Selector(*SYMBOL_RESET),
            Bold                    => TalkValue::Selector(*SYMBOL_BOLD),
            Dim                     => TalkValue::Selector(*SYMBOL_DIM),
            Italic                  => TalkValue::Selector(*SYMBOL_ITALIC),
            Underlined              => TalkValue::Selector(*SYMBOL_UNDERLINED),
            DoubleUnderlined        => TalkValue::Selector(*SYMBOL_DOUBLEUNDERLINED),
            Undercurled             => TalkValue::Selector(*SYMBOL_UNDERCURLED),
            Underdotted             => TalkValue::Selector(*SYMBOL_UNDERDOTTED),
            Underdashed             => TalkValue::Selector(*SYMBOL_UNDERDASHED),
            SlowBlink               => TalkValue::Selector(*SYMBOL_SLOWBLINK),
            RapidBlink              => TalkValue::Selector(*SYMBOL_RAPIDBLINK),
            Reverse                 => TalkValue::Selector(*SYMBOL_REVERSE),
            Hidden                  => TalkValue::Selector(*SYMBOL_HIDDEN),
            CrossedOut              => TalkValue::Selector(*SYMBOL_CROSSEDOUT),
            Fraktur                 => TalkValue::Selector(*SYMBOL_FRAKTUR),
            NoBold                  => TalkValue::Selector(*SYMBOL_NOBOLD),
            NormalIntensity         => TalkValue::Selector(*SYMBOL_NORMALINTENSITY),
            NoItalic                => TalkValue::Selector(*SYMBOL_NOITALIC),
            NoUnderline             => TalkValue::Selector(*SYMBOL_NOUNDERLINE),
            NoBlink                 => TalkValue::Selector(*SYMBOL_NOBLINK),
            NoReverse               => TalkValue::Selector(*SYMBOL_NOREVERSE),
            NoHidden                => TalkValue::Selector(*SYMBOL_NOHIDDEN),
            NotCrossedOut           => TalkValue::Selector(*SYMBOL_NOTCROSSEDOUT),
            Framed                  => TalkValue::Selector(*SYMBOL_FRAMED),
            Encircled               => TalkValue::Selector(*SYMBOL_ENCIRCLED),
            OverLined               => TalkValue::Selector(*SYMBOL_OVERLINED),
            NotFramedOrEncircled    => TalkValue::Selector(*SYMBOL_NOTFRAMEDORENCIRCLED),
            NotOverLined            => TalkValue::Selector(*SYMBOL_NOTOVERLINED),
        };

        TalkOwned::new(value, context)
    }

    fn try_from_talk_value<'a>(value: TalkOwned<TalkValue, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        use TalkStyleAttribute::*;

        match &*value {
            TalkValue::Selector(msg_id) => {
                if *msg_id == *SYMBOL_RESET                     { Ok(Reset) }
                else if *msg_id == *SYMBOL_BOLD                 { Ok(Bold) }
                else if *msg_id == *SYMBOL_DIM                  { Ok(Dim) }
                else if *msg_id == *SYMBOL_ITALIC               { Ok(Italic) }
                else if *msg_id == *SYMBOL_UNDERLINED           { Ok(Underlined) }
                else if *msg_id == *SYMBOL_DOUBLEUNDERLINED     { Ok(DoubleUnderlined) }
                else if *msg_id == *SYMBOL_UNDERCURLED          { Ok(Undercurled) }
                else if *msg_id == *SYMBOL_UNDERDOTTED          { Ok(Underdotted) }
                else if *msg_id == *SYMBOL_UNDERDASHED          { Ok(Underdashed) }
                else if *msg_id == *SYMBOL_SLOWBLINK            { Ok(SlowBlink) }
                else if *msg_id == *SYMBOL_RAPIDBLINK           { Ok(RapidBlink) }
                else if *msg_id == *SYMBOL_REVERSE              { Ok(Reverse) }
                else if *msg_id == *SYMBOL_HIDDEN               { Ok(Hidden) }
                else if *msg_id == *SYMBOL_CROSSEDOUT           { Ok(CrossedOut) }
                else if *msg_id == *SYMBOL_FRAKTUR              { Ok(Fraktur) }
                else if *msg_id == *SYMBOL_NOBOLD               { Ok(NoBold) }
                else if *msg_id == *SYMBOL_NORMALINTENSITY      { Ok(NormalIntensity) }
                else if *msg_id == *SYMBOL_NOITALIC             { Ok(NoItalic) }
                else if *msg_id == *SYMBOL_NOUNDERLINE          { Ok(NoUnderline) }
                else if *msg_id == *SYMBOL_NOBLINK              { Ok(NoBlink) }
                else if *msg_id == *SYMBOL_NOREVERSE            { Ok(NoReverse) }
                else if *msg_id == *SYMBOL_NOHIDDEN             { Ok(NoHidden) }
                else if *msg_id == *SYMBOL_NOTCROSSEDOUT        { Ok(NotCrossedOut) }
                else if *msg_id == *SYMBOL_FRAMED               { Ok(Framed) }
                else if *msg_id == *SYMBOL_ENCIRCLED            { Ok(Encircled) }
                else if *msg_id == *SYMBOL_OVERLINED            { Ok(OverLined) }
                else if *msg_id == *SYMBOL_NOTFRAMEDORENCIRCLED { Ok(NotFramedOrEncircled) }
                else if *msg_id == *SYMBOL_NOTOVERLINED         { Ok(NotOverLined) }
                else                                            { Err(TalkError::UnexpectedSelector(*msg_id)) }
            }

            _ => Err(TalkError::NotASelector)
        }
    }
}

#[cfg(feature="crossterm")]
impl Into<crossterm::style::Attribute> for TalkStyleAttribute {
    fn into(self) -> crossterm::style::Attribute {
        match self {
            TalkStyleAttribute::Reset                   => crossterm::style::Attribute::Reset,
            TalkStyleAttribute::Bold                    => crossterm::style::Attribute::Bold,
            TalkStyleAttribute::Dim                     => crossterm::style::Attribute::Dim,
            TalkStyleAttribute::Italic                  => crossterm::style::Attribute::Italic,
            TalkStyleAttribute::Underlined              => crossterm::style::Attribute::Underlined,
            TalkStyleAttribute::DoubleUnderlined        => crossterm::style::Attribute::DoubleUnderlined,
            TalkStyleAttribute::Undercurled             => crossterm::style::Attribute::Undercurled,
            TalkStyleAttribute::Underdotted             => crossterm::style::Attribute::Underdotted,
            TalkStyleAttribute::Underdashed             => crossterm::style::Attribute::Underdashed,
            TalkStyleAttribute::SlowBlink               => crossterm::style::Attribute::SlowBlink,
            TalkStyleAttribute::RapidBlink              => crossterm::style::Attribute::RapidBlink,
            TalkStyleAttribute::Reverse                 => crossterm::style::Attribute::Reverse,
            TalkStyleAttribute::Hidden                  => crossterm::style::Attribute::Hidden,
            TalkStyleAttribute::CrossedOut              => crossterm::style::Attribute::CrossedOut,
            TalkStyleAttribute::Fraktur                 => crossterm::style::Attribute::Fraktur,
            TalkStyleAttribute::NoBold                  => crossterm::style::Attribute::NoBold,
            TalkStyleAttribute::NormalIntensity         => crossterm::style::Attribute::NormalIntensity,
            TalkStyleAttribute::NoItalic                => crossterm::style::Attribute::NoItalic,
            TalkStyleAttribute::NoUnderline             => crossterm::style::Attribute::NoUnderline,
            TalkStyleAttribute::NoBlink                 => crossterm::style::Attribute::NoBlink,
            TalkStyleAttribute::NoReverse               => crossterm::style::Attribute::NoReverse,
            TalkStyleAttribute::NoHidden                => crossterm::style::Attribute::NoHidden,
            TalkStyleAttribute::NotCrossedOut           => crossterm::style::Attribute::NotCrossedOut,
            TalkStyleAttribute::Framed                  => crossterm::style::Attribute::Framed,
            TalkStyleAttribute::Encircled               => crossterm::style::Attribute::Encircled,
            TalkStyleAttribute::OverLined               => crossterm::style::Attribute::OverLined,
            TalkStyleAttribute::NotFramedOrEncircled    => crossterm::style::Attribute::NotFramedOrEncircled,
            TalkStyleAttribute::NotOverLined            => crossterm::style::Attribute::NotOverLined,
        }
    }
}

#[cfg(feature="crossterm")]
impl Into<crossterm::style::Color> for TalkStyleColor {
    fn into(self) -> crossterm::style::Color {
        match self {
            TalkStyleColor::Reset           => crossterm::style::Color::Reset,
            TalkStyleColor::Black           => crossterm::style::Color::Black,
            TalkStyleColor::DarkGrey        => crossterm::style::Color::DarkGrey,
            TalkStyleColor::Red             => crossterm::style::Color::Red,
            TalkStyleColor::DarkRed         => crossterm::style::Color::DarkRed,
            TalkStyleColor::Green           => crossterm::style::Color::Green,
            TalkStyleColor::DarkGreen       => crossterm::style::Color::DarkGreen,
            TalkStyleColor::Yellow          => crossterm::style::Color::Yellow,
            TalkStyleColor::DarkYellow      => crossterm::style::Color::DarkYellow,
            TalkStyleColor::Blue            => crossterm::style::Color::Blue,
            TalkStyleColor::DarkBlue        => crossterm::style::Color::DarkBlue,
            TalkStyleColor::Magenta         => crossterm::style::Color::Magenta,
            TalkStyleColor::DarkMagenta     => crossterm::style::Color::DarkMagenta,
            TalkStyleColor::Cyan            => crossterm::style::Color::Cyan,
            TalkStyleColor::DarkCyan        => crossterm::style::Color::DarkCyan,
            TalkStyleColor::White           => crossterm::style::Color::White,
            TalkStyleColor::Grey            => crossterm::style::Color::Grey,
            TalkStyleColor::Rgb { r, g, b } => crossterm::style::Color::Rgb { r: r as _, g: g as _, b: b as _ },
        }
    }
}

///
/// TalkTerminalOut just uses the messages from its 'subtypes' directly
///
impl TalkMessageType for TalkTerminalOut {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        TalkSimpleStreamRequest::supports_message(id)
            || TalkTerminalCmd::supports_message(id)
            || TalkTextStyleCmd::supports_message(id)
            || TalkCursorCmd::supports_message(id)
    }

    /// Converts a message to an object of this type
    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        use TalkTerminalOut::*;

        if TalkSimpleStreamRequest::supports_message(message.signature_id()) {
            
            Ok(Put(TalkSimpleStreamRequest::from_message(message, context)?))
        
        } else if TalkTerminalCmd::supports_message(message.signature_id()) {
            
            Ok(Terminal(TalkTerminalCmd::from_message(message, context)?))

        } else if TalkTextStyleCmd::supports_message(message.signature_id()) {

            Ok(Style(TalkTextStyleCmd::from_message(message, context)?))

        } else if TalkCursorCmd::supports_message(message.signature_id()) {

            Ok(Cursor(TalkCursorCmd::from_message(message, context)?))

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
            Cursor(cursor)      => cursor.to_message(context),
        }
    }
}

#[cfg(feature="crossterm")]
fn crossterm_put(req: TalkSimpleStreamRequest, output: &mut (impl Write + crossterm::QueueableCommand)) {
    match req {
        TalkSimpleStreamRequest::Write(string)  => { output.queue(crossterm::style::Print(string)).ok(); output.flush().ok(); }
        TalkSimpleStreamRequest::WriteChr(chr)  => { output.queue(crossterm::style::Print(chr)).ok(); }
        TalkSimpleStreamRequest::Flush          => { output.flush().ok(); }
    }
}

#[cfg(feature="crossterm")]
fn crossterm_terminal(req: TalkTerminalCmd, output: &mut (impl Write + crossterm::QueueableCommand)) {
    match req {
        TalkTerminalCmd::ClearAll               => { output.queue(crossterm::terminal::Clear(crossterm::terminal::ClearType::All)).ok(); }
        TalkTerminalCmd::DisableLineWrap        => { output.queue(crossterm::terminal::DisableLineWrap).ok(); }
        TalkTerminalCmd::EnableLineWrap         => { output.queue(crossterm::terminal::EnableLineWrap).ok(); }
        TalkTerminalCmd::EnterAlternateScreen   => { output.queue(crossterm::terminal::EnterAlternateScreen).ok(); }
        TalkTerminalCmd::LeaveAlternateScreen   => { output.queue(crossterm::terminal::LeaveAlternateScreen).ok(); }
        TalkTerminalCmd::ScrollDown(lines)      => { output.queue(crossterm::terminal::ScrollDown(lines as _)).ok(); }
        TalkTerminalCmd::ScrollUp(lines)        => { output.queue(crossterm::terminal::ScrollUp(lines as _)).ok(); }
        TalkTerminalCmd::SetSize(w, h)          => { output.queue(crossterm::terminal::SetSize(w as _, h as _)).ok(); }
        TalkTerminalCmd::SetTitle(msg)          => { output.queue(crossterm::terminal::SetTitle(msg)).ok(); }
    }
}

#[cfg(feature="crossterm")]
fn crossterm_style(req: TalkTextStyleCmd, output: &mut (impl Write + crossterm::QueueableCommand)) {
    match req {
        TalkTextStyleCmd::SetAttribute(style)       => { output.queue(crossterm::style::SetAttribute(style.into())).ok(); }
        TalkTextStyleCmd::SetForeground(col)        => { output.queue(crossterm::style::SetForegroundColor(col.into())).ok(); }
        TalkTextStyleCmd::SetBackground(col)        => { output.queue(crossterm::style::SetBackgroundColor(col.into())).ok(); }
        TalkTextStyleCmd::SetColors(fg_col, bg_col) => { output.queue(crossterm::style::SetColors(crossterm::style::Colors::new(fg_col.into(), bg_col.into()))).ok(); }
        TalkTextStyleCmd::SetUnderlineColor(col)    => { output.queue(crossterm::style::SetUnderlineColor(col.into())).ok(); }
    }
}

#[cfg(feature="crossterm")]
fn crossterm_cursor(req: TalkCursorCmd, output: &mut (impl Write + crossterm::QueueableCommand)) {
    match req {
        TalkCursorCmd::DisableBlinking          => { output.queue(crossterm::cursor::DisableBlinking).ok(); },
        TalkCursorCmd::EnableBlinking           => { output.queue(crossterm::cursor::EnableBlinking).ok(); },
        TalkCursorCmd::Hide                     => { output.queue(crossterm::cursor::Hide).ok(); },
        TalkCursorCmd::Show                     => { output.queue(crossterm::cursor::Show).ok(); },
        TalkCursorCmd::MoveDown(n)              => { output.queue(crossterm::cursor::MoveDown(n as _)).ok(); },
        TalkCursorCmd::MoveUp(n)                => { output.queue(crossterm::cursor::MoveUp(n as _)).ok(); },
        TalkCursorCmd::MoveLeft(n)              => { output.queue(crossterm::cursor::MoveLeft(n as _)).ok(); },
        TalkCursorCmd::MoveRight(n)             => { output.queue(crossterm::cursor::MoveRight(n as _)).ok(); },
        TalkCursorCmd::MoveTo(x, y)             => { output.queue(crossterm::cursor::MoveTo(x as _, y as _)).ok(); },
        TalkCursorCmd::MoveToColumn(n)          => { output.queue(crossterm::cursor::MoveToColumn(n as _)).ok(); },
        TalkCursorCmd::MoveToRow(n)             => { output.queue(crossterm::cursor::MoveToRow(n as _)).ok(); },
        TalkCursorCmd::MoveToNextLine(n)        => { output.queue(crossterm::cursor::MoveToNextLine(n as _)).ok(); },
        TalkCursorCmd::MoveToPreviousLine(n)    => { output.queue(crossterm::cursor::MoveToPreviousLine(n as _)).ok(); },
        TalkCursorCmd::RestorePosition          => { output.queue(crossterm::cursor::RestorePosition).ok(); },
        TalkCursorCmd::SavePosition             => { output.queue(crossterm::cursor::SavePosition).ok(); },
    }
}

///
/// Processes a stream of commands destined for crossterm
///
#[cfg(feature="crossterm")]
pub async fn talk_process_crossterm_output(stream: impl Send + Stream<Item=TalkTerminalOut>, output: impl Send + Write + crossterm::QueueableCommand) {
    let mut output = output;
    pin_mut!(stream);

    while let Some(cmd) = stream.next().await {
        use TalkTerminalOut::*;

        match cmd {
            Put(put)            => crossterm_put(put, &mut output), 
            Terminal(terminal)  => crossterm_terminal(terminal, &mut output),
            Style(style)        => crossterm_style(style, &mut output),
            Cursor(cursor)      => crossterm_cursor(cursor, &mut output),
        }
    }
}

///
/// Creates a continuation that returns a crossterm terminal object
///
#[cfg(feature="crossterm")]
pub fn talk_crossterm_terminal(output: impl 'static + Send + Write + crossterm::QueueableCommand) -> TalkContinuation<'static> {
    TalkContinuation::soon(move |talk_context| {
        // Create the channel to send the requests
        let (send, receive) = create_talk_sender_in_context(talk_context);
        let send            = send.leak();

        // Run the processing receiver as a background task in the context
        let receive = TalkContinuation::future_value(async move {
            talk_process_crossterm_output(receive, output).await;
            ().into()
        });
        talk_context.run_in_background(receive);

        // Result is the sender object
        send.into()
    })
}
