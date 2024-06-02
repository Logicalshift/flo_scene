use crate::command_stream::*;
use crate::parser::*;
use crate::parse_json::*;

use futures::prelude::*;
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

impl TokenMatcher<CommandToken> for CommandToken {
    fn try_match(&self, lookahead: &'_ str, eof: bool) -> TokenMatchResult<CommandToken> {
        match self {
            CommandToken::Command   => match_command(lookahead, eof),
            CommandToken::Comment   => match_command_comment(lookahead, eof),
            CommandToken::Pipe      => if lookahead.starts_with("|") { TokenMatchResult::Matches(CommandToken::Pipe, 1) } else { TokenMatchResult::LookaheadCannotMatch },
            CommandToken::SemiColon => if lookahead.starts_with(";") { TokenMatchResult::Matches(CommandToken::SemiColon, 1) } else { TokenMatchResult::LookaheadCannotMatch },
            CommandToken::Equals    => if lookahead.starts_with("=") { TokenMatchResult::Matches(CommandToken::Equals, 1) } else { TokenMatchResult::LookaheadCannotMatch },
            CommandToken::Json(_)   => TokenMatchResult::LookaheadCannotMatch
        }
    }
}

impl<TStream> Tokenizer<CommandToken, TStream> {
    ///
    /// Adds the set of JSON token matchers to this tokenizer
    ///
    pub fn with_command_matchers(&mut self) -> &mut Self {
        self
            .with_matcher(CommandToken::Command)
            .with_matcher(CommandToken::Comment)
            .with_matcher(CommandToken::Pipe)
            .with_matcher(CommandToken::SemiColon)
            .with_matcher(CommandToken::Equals)
            .with_json_matchers();

        self
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

///
/// Parses a command from an input stream
///
pub async fn command_parse<TStream>(parser: &mut Parser<TokenMatch<CommandToken>, Command>, tokenizer: &mut Tokenizer<CommandToken, TStream>) -> Result<(), ()> 
where
    TStream: Stream<Item=Vec<u8>>,
{
    todo!()
}

///
/// Parses an argument to a command (resulting in a Command::Command with no name)
///
async fn command_parse_argument<TStream>(parser: &mut Parser<TokenMatch<CommandToken>, Command>, tokenizer: &mut Tokenizer<CommandToken, TStream>) -> Result<(), ()> 
where
    TStream: Stream<Item=Vec<u8>>,
{
    // Create a JSON parser to read the following JSON value
    let mut json_parser = Parser::with_lookahead_from(parser);
    json_parse_value(&mut json_parser, tokenizer).await?;

    // Restore any lookahead to the original parser
    parser.take_lookahead_from(&mut json_parser);

    // Fetch the JSON value for the argument
    let json_value = json_parser.finish().map_err(|_| ())?;

    // Add as a node to the current parser
    parser.reduce(0, |_| Command::Command { command: CommandName("".to_string()), argument: json_value }).map_err(|_| ())?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tokenizer::*;

    use serde_json::*;
    use futures::executor;

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

    #[test]
    fn parse_argument() {
        let argument        = stream::iter(r#"[ 1, 2, 3, 4 ]"#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse_argument(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == Command::Command { command: CommandName("".to_string()), argument: json!{[1, 2, 3, 4]} });
        });
    }
}
