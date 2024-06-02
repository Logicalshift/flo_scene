use crate::command_stream::*;
use crate::parser::*;
use crate::parse_json::*;

use futures::prelude::*;
use regex_automata::dfa::dense;
use once_cell::sync::{Lazy};

static COMMAND: Lazy<dense::DFA<Vec<u32>>> = Lazy::new(|| dense::DFA::new(r"^(\p{L}|[_:-])(\p{L}|\p{N}|[_:-])*").unwrap());
static COMMENT: Lazy<dense::DFA<Vec<u32>>> = Lazy::new(|| dense::DFA::new(r"^(//[^\r\n]*)").unwrap());

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

    /// Whitespace ending in a newline (also used to end a command)
    Newline,

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
            CommandToken::Newline       => Ok(JsonToken::Whitespace),
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
            CommandToken::Newline   => {
                match match_whitespace(lookahead, eof) {
                    TokenMatchResult::Matches(JsonToken::Whitespace, count) => {
                        if lookahead.as_bytes()[count-1] == b'\n' || lookahead.as_bytes()[count-1] == b'\r' {
                            TokenMatchResult::Matches(CommandToken::Newline, count)
                        } else {
                            TokenMatchResult::LookaheadCannotMatch
                        }
                    }

                    TokenMatchResult::Matches(_, _) => {
                        unreachable!()
                    }

                    TokenMatchResult::LookaheadCannotMatch => {
                        TokenMatchResult::LookaheadCannotMatch
                    }

                    TokenMatchResult::LookaheadIsPrefix => {
                        TokenMatchResult::LookaheadIsPrefix
                    }
                }
            }
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
            .with_json_matchers()
            .with_matcher(CommandToken::Command)
            .with_matcher(CommandToken::Comment)
            .with_matcher(CommandToken::Pipe)
            .with_matcher(CommandToken::SemiColon)
            .with_matcher(CommandToken::Equals)
            .with_matcher(CommandToken::Newline);

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
/// Reads a command token from the tokenizer
///
pub async fn command_read_token<TStream>(tokenizer: &mut Tokenizer<CommandToken, TStream>) -> Option<TokenMatch<CommandToken>>
where
    TStream: Stream<Item=Vec<u8>>,
{
    loop {
        // Acquire a token from the tokenizer
        let next_match = tokenizer.match_token().await?;

        // Skip over whitespace, then return the first 'sold' value
        match next_match.token {
            Some(CommandToken::Json(JsonToken::Whitespace)) => { }
            _ => { break Some(next_match); }
        }
    }
}

///
/// Parses a command from an input stream
///
pub async fn command_parse<TStream>(parser: &mut Parser<TokenMatch<CommandToken>, Command>, tokenizer: &mut Tokenizer<CommandToken, TStream>) -> Result<(), ()> 
where
    TStream: Stream<Item=Vec<u8>>,
{
    loop {
        let lookahead = parser.lookahead(0, tokenizer, |tokenizer| command_read_token(tokenizer).boxed_local()).await;

        if let Some(lookahead) = lookahead {
            match lookahead.token {
                Some(CommandToken::Newline) => { parser.skip_token(); }
                Some(CommandToken::Command) => { command_parse_command(parser, tokenizer).await?; break Ok(()); }

                _ => { break Err(()); }
            }
        } else {
            // No symbol
            break Err(());
        }
    }
}

///
/// Parses a command, at the point where the lookahead contains the 'Command' token
///
///
async fn command_parse_command<TStream>(parser: &mut Parser<TokenMatch<CommandToken>, Command>, tokenizer: &mut Tokenizer<CommandToken, TStream>) -> Result<(), ()>
where
    TStream: Stream<Item=Vec<u8>>,
 {
    // Lookahead must be a 'Command'
    let command_name = parser.lookahead(0, tokenizer, |tokenizer| command_read_token(tokenizer).boxed_local()).await.ok_or(())?;
    if command_name.token != Some(CommandToken::Command) { return Err(()); }

    parser.accept_token().map_err(|_| ())?;

    // Next lookahead determines the type of command
    let maybe_argument = parser.lookahead(0, tokenizer, |tokenizer| command_read_token(tokenizer).boxed_local()).await;
    if let Some(maybe_argument) = maybe_argument {
        match maybe_argument.token {
            Some(CommandToken::Json(_)) => {
                // Argument is a JSON value which may be followed by a pipe or an equals
                command_parse_argument(parser, tokenizer).await?;

                parser.reduce(2, |cmd| {
                    let name        = cmd[0].token().unwrap().fragment.clone();
                    let argument    = cmd[1].node().unwrap().clone();

                    match argument {
                        Command::Command { argument, .. }   => Command::Command { command: CommandName(name), argument: argument },
                        _                                   => { unreachable!() }
                    }
                }).map_err(|_| ())?;

                // TODO: command can use pipe or an equals here
            }

            Some(CommandToken::Newline)     |
            Some(CommandToken::SemiColon)   => {
                // Command has no argument
                parser.accept_token().map_err(|_| ())?;

                parser.reduce(2, |cmd| {
                    let name = cmd[0].token().unwrap().fragment.clone();
                    Command::Command { command: CommandName(name), argument: serde_json::Value::Null }
                }).map_err(|_| ())?;
            }

            // TODO: command can have no arguments and be followed by a pipe or an equals

            _ => { return Err(()); }
        }
    } else {
        // No argument, so just a command
        parser.reduce(1, |cmd| {
            let name = cmd[0].token().unwrap().fragment.clone();
            Command::Command { command: CommandName(name), argument: serde_json::Value::Null }
        }).map_err(|_| ())?;
    }

    Ok(())
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
    fn tokenize_newline() {
        let whitespace      = "    \n    ";
        let mut tokenizer   = Tokenizer::new(stream::iter(whitespace.bytes()).ready_chunks(2));

        tokenizer.with_command_matchers();

        executor::block_on(async move {
            let newline     = command_read_token(&mut tokenizer).await;
            let eof         = command_read_token(&mut tokenizer).await;

            assert!(newline.unwrap().token == Some(CommandToken::Newline));
            assert!(eof.is_none());
        });
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

    #[test]
    fn parse_command_without_arguments() {
        let argument        = stream::iter(r#"some::command"#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == Command::Command { command: CommandName("some::command".to_string()), argument: serde_json::Value::Null });
        });
    }

    #[test]
    fn parse_command_with_arguments() {
        let argument        = stream::iter(r#"some::command [ 1, 2, 3, 4 ]"#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == Command::Command { command: CommandName("some::command".to_string()), argument: json!{[1, 2, 3, 4]} });
        });
    }

    #[test]
    fn parse_command_with_arguments_and_newline() {
        let argument        = stream::iter("some::command [ 1, 2, 3, 4 ]\n".bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == Command::Command { command: CommandName("some::command".to_string()), argument: json!{[1, 2, 3, 4]} });
        });
    }

    #[test]
    fn parse_command_with_arguments_and_following_data() {
        let argument        = stream::iter("some::command [ 1, 2, 3, 4 ]\nfoo".bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == Command::Command { command: CommandName("some::command".to_string()), argument: json!{[1, 2, 3, 4]} });
        });
    }

    #[test]
    fn parse_several_commands() {
        let argument        = stream::iter(r#"
            some::command [ 1, 2, 3, 4 ]
            another::command
            one_more ; and_another [ "Hello" ]
            "#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();
            assert!(result == Command::Command { command: CommandName("some::command".to_string()), argument: json!{[1, 2, 3, 4]} });

            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();
            assert!(result == Command::Command { command: CommandName("another::command".to_string()), argument: serde_json::Value::Null });

            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();
            assert!(result == Command::Command { command: CommandName("one_more".to_string()), argument: serde_json::Value::Null });

            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();
            assert!(result == Command::Command { command: CommandName("and_another".to_string()), argument: json!{["Hello"]} });
        });
    }
}
