use crate::parser::*;
use crate::parse_json::*;

use regex_automata::dfa::dense;
use once_cell::sync::{Lazy};

static COMMAND: Lazy<dense::DFA<Vec<u32>>> = Lazy::new(|| dense::DFA::new(r"(\p{L}|[_:-])(\p{L}|\p{N}|[_:-])*").unwrap());
static COMMENT: Lazy<dense::DFA<Vec<u32>>> = Lazy::new(|| dense::DFA::new(r"(//[^\r\n]*)").unwrap());

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

    /// A '// comment'
    Comment,

    /// A JSON token
    Json(JsonToken),
}

impl From<JsonToken> for CommandToken {
    #[inline]
    fn from(token: JsonToken) -> Self {
        CommandToken::Json(token)
    }
}

impl TryInto<JsonToken> for CommandToken {
    type Error = Self;

    #[inline]
    fn try_into(self) -> Result<JsonToken, Self::Error> {
        match self {
            CommandToken::Json(token)   => Ok(token),
            CommandToken::Comment       => Ok(JsonToken::Whitespace),
            other                       => Err(other),
        }
    }
}

///
/// Matches against the command syntax
///
fn match_command(lookahead: &str, eof: bool) -> TokenMatchResult<CommandToken> {
    match_regex(&*COMMAND, lookahead, eof).with_token(CommandToken::Command)
}

///
/// Matches against the comment syntax
///
fn match_command_comment(lookahead: &str, eof: bool) -> TokenMatchResult<CommandToken> {
    match_regex(&*COMMENT, lookahead, eof).with_token(CommandToken::Comment)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn match_simple_command() {
        let match_result = match_command("test::command", true);
        assert!(match_result == TokenMatchResult::Matches(CommandToken::Command, "test::command".chars().count()));
    }

    #[test]
    fn match_simple_comment() {
        let match_result = match_command_comment("// comment", true);
        assert!(match_result == TokenMatchResult::Matches(CommandToken::Comment, "// comment".chars().count()));
    }
}
