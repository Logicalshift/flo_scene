use crate::parser::*;
use crate::parse_json::*;

use regex_automata::dfa::dense;
use once_cell::sync::{Lazy};

static COMMAND: Lazy<dense::DFA<Vec<u32>>> = Lazy::new(|| dense::DFA::new(r"(\p{L}|[_:-])(\p{L}|\p{N}|[_:-])*").unwrap());

///
/// Tokens from the command stream
///
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CommandToken {
    /// Specifies which command to call
    Command,

    /// The '|' symbol, used to send the output of one command to another
    Pipe,

    /// The ';' symbol, used to end a command
    SemiColon,

    /// The '=' symbol, used to record a command result in a variable
    Equals,

    /// A '// comment' or a '/* comment */'
    Comment,

    /// A JSON token
    Json(JsonToken),
}

///
/// Matches against the command syntax
///
fn match_command(lookahead: &str, eof: bool) -> TokenMatchResult<CommandToken> {
    match_regex(&*COMMAND, lookahead, eof).with_token(CommandToken::Command)
}
