use super::command_stream::*;
use crate::parser::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

///
/// Tokens from the command stream
///
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CommandToken {
    /// Specifies which command to call
    Command,

    /// A variable name
    Variable,

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

///
/// The errors that can happen while parsing a command
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandParseError {
    /// Error while parsing a JSON argument
    JsonError(JsonParseError),

    /// The lookahead wasn't expected at this point
    UnexpectedToken(Option<CommandToken>, String),

    /// Ran out of input while parsing the command
    ExpectedMoreInput,

    /// Usually an error in the parser, we tried to 'reduce' a token when we hadn't previously accepted enough input 
    ParserStackTooSmall,

    /// Usually indicates an error with the parser, we failed to 'converge' to a single value
    ParserDidNotConverge,
}

impl<'a, TToken> From<&'a TokenMatch<TToken>> for CommandParseError 
where
    TToken: Clone + TryInto<CommandToken>,
{
    fn from(err_lookahead: &'a TokenMatch<TToken>) -> CommandParseError {
        let json_token = err_lookahead.token.clone().map(|token| token.try_into());

        match json_token {
            Some(token) => CommandParseError::UnexpectedToken(token.ok(), err_lookahead.fragment.clone()),
            None        => CommandParseError::UnexpectedToken(None, err_lookahead.fragment.clone()),
        }
    }
}

impl From<ParserLookaheadEmpty> for CommandParseError {
    fn from(_err: ParserLookaheadEmpty) -> CommandParseError {
        CommandParseError::ExpectedMoreInput
    }
}

impl From<ParserStackTooSmall> for CommandParseError {
    fn from(_err: ParserStackTooSmall) -> CommandParseError {
        CommandParseError::ParserStackTooSmall
    }
}

impl From<ParserDidNotConverge> for CommandParseError {
    fn from(_err: ParserDidNotConverge) -> CommandParseError {
        CommandParseError::ParserDidNotConverge
    }
}

impl From<JsonParseError> for CommandParseError {
    fn from(err: JsonParseError) -> CommandParseError {
        CommandParseError::JsonError(err)
    }
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
            CommandToken::Variable      => Ok(JsonToken::Variable),
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
            CommandToken::Variable  => match_variable(lookahead, eof),
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
            .with_matcher(CommandToken::Variable)
            .with_matcher(CommandToken::Comment)
            .with_matcher(CommandToken::Pipe)
            .with_matcher(CommandToken::SemiColon)
            .with_matcher(CommandToken::Equals)
            .with_matcher(CommandToken::Newline);

        self
    }
}

///
/// Matches against the command token
///
fn match_command(lookahead: &str, eof: bool) -> TokenMatchResult<CommandToken> {
    let mut characters = lookahead.chars();
    let mut len = 0;

    if let Some(first_chr) = characters.next() {
        if first_chr.is_alphabetic() || first_chr == '_' || first_chr == ':' {
            // Will match a command of some description
            len += 1;

            while let Some(next_chr) = characters.next() {
                if next_chr.is_alphabetic() || next_chr.is_digit(10) || next_chr == '_' || next_chr == ':' {
                    // Is a valid continuation
                } else {
                    if first_chr != ':' || len > 1 {
                        return TokenMatchResult::Matches(CommandToken::Command, len);
                    } else {
                        return TokenMatchResult::LookaheadCannotMatch;
                    }
                }

                len += 1;
            }

            if eof {
                TokenMatchResult::Matches(CommandToken::Command, len)
            } else {
                TokenMatchResult::LookaheadIsPrefix
            }
        } else {
            // Not a command
            TokenMatchResult::LookaheadCannotMatch
        }
    } else {
        TokenMatchResult::LookaheadCannotMatch
    }
}

///
/// Matches against the variable token
///
/// Variables are `:name`, `$name` or `_name`
///
fn match_variable(lookahead: &str, eof: bool) -> TokenMatchResult<CommandToken> {
    let mut characters = lookahead.chars();
    let mut len = 0;

    if let Some(first_chr) = characters.next() {
        if first_chr == ':' || first_chr == '$' || first_chr == '_' {
            // Will match a command of some description
            len += 1;

            while let Some(next_chr) = characters.next() {
                if next_chr.is_alphabetic() || next_chr.is_digit(10) || next_chr == '_' || next_chr == ':' {
                    // Is a valid continuation
                } else {
                    if len > 1 {
                        return TokenMatchResult::Matches(CommandToken::Variable, len);
                    } else {
                        return TokenMatchResult::LookaheadCannotMatch;
                    }
                }

                len += 1;
            }

            if eof {
                TokenMatchResult::Matches(CommandToken::Variable, len)
            } else {
                TokenMatchResult::LookaheadIsPrefix
            }
        } else {
            // Not a command
            TokenMatchResult::LookaheadCannotMatch
        }
    } else {
        TokenMatchResult::LookaheadCannotMatch
    }
}

///
/// Matches against the comment syntax
///
fn match_command_comment(lookahead: &str, eof: bool) -> TokenMatchResult<CommandToken> {
    let mut chrs = lookahead.chars();

    if let Some(chr) = chrs.next() {
        // Starts with '//'
        if chr != '/' { return TokenMatchResult::LookaheadCannotMatch; }

        if let Some(chr) = chrs.next() {
            if chr != '/' { return TokenMatchResult::LookaheadCannotMatch; }
        } else if !eof {
            return TokenMatchResult::LookaheadIsPrefix;
        } else {
            return TokenMatchResult::LookaheadCannotMatch;
        }

        // Everthing up to the next '\n' matches
        let mut len = 2;
        while let Some(chr) = chrs.next() {
            if chr == '\n' || chr == '\r' {
                return TokenMatchResult::Matches(CommandToken::Comment, len+1);
            }

            len += 1;
        }

        if !eof {
            TokenMatchResult::LookaheadIsPrefix
        } else {
            return TokenMatchResult::Matches(CommandToken::Comment, len);
        }
    } else {
        // Empty string can be a prefix of anything
        TokenMatchResult::LookaheadIsPrefix
    }
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
pub fn command_parse<'a, TStream>(parser: &'a mut Parser<TokenMatch<CommandToken>, CommandRequest>, tokenizer: &'a mut Tokenizer<CommandToken, TStream>) -> BoxFuture<'a, Result<(), CommandParseError>>
where
    TStream: Send + Stream<Item=Vec<u8>>,
{
    async move {
        loop {
            let lookahead = parser.lookahead(0, tokenizer, |tokenizer| command_read_token(tokenizer).boxed()).await;

            if let Some(lookahead) = lookahead {
                match lookahead.token {
                    Some(CommandToken::Newline)  => { parser.skip_token(); }
                    Some(CommandToken::Command)  => { command_parse_command(parser, tokenizer).await?; break Ok(()); }
                    Some(CommandToken::Variable) => { 
                        let maybe_equals = parser.lookahead(1, tokenizer, |tokenizer| command_read_token(tokenizer).boxed()).await;

                        if maybe_equals.as_ref().and_then(|eqls| eqls.token) == Some(CommandToken::Equals) {
                            command_parse_assignment(parser, tokenizer).await?; 
                            break Ok(());
                        } else {
                            command_parse_command(parser, tokenizer).await?;
                            break Ok(());
                        }
                    }

                    Some(CommandToken::Json(_)) => {
                        // Convert to a JSON parser
                        let mut json_parser = Parser::with_lookahead_from(parser);
                        json_parse_value(&mut json_parser, tokenizer).await?;

                        // Restore any lookahead to the original parser
                        parser.take_lookahead_from(&mut json_parser);

                        // Fetch the JSON value for the argument
                        let json_value = json_parser.finish()?;
                        parser.reduce(0, |_| CommandRequest::RawJson { value: json_value.into() })?;

                        break Ok(());
                    }

                    _ => { break Err(lookahead.into()); }
                }
            } else {
                // No symbol
                break Err(CommandParseError::ExpectedMoreInput);
            }
        }
    }.boxed()
}

///
/// Parses a command, at the point where the lookahead contains the 'Command' token
///
async fn command_parse_command<TStream>(parser: &mut Parser<TokenMatch<CommandToken>, CommandRequest>, tokenizer: &mut Tokenizer<CommandToken, TStream>) -> Result<(), CommandParseError>
where
    TStream: Send + Stream<Item=Vec<u8>>,
 {
    // Lookahead must be a 'Command'
    let command_name = parser.lookahead(0, tokenizer, |tokenizer| command_read_token(tokenizer).boxed()).await.ok_or(CommandParseError::ExpectedMoreInput)?;
    if !matches!(command_name.token, Some(CommandToken::Command) | Some(CommandToken::Variable)) { return Err(command_name.into()); }

    parser.accept_token()?;

    // Next lookahead determines the type of command
    let maybe_argument = parser.lookahead(0, tokenizer, |tokenizer| command_read_token(tokenizer).boxed()).await;
    if let Some(maybe_argument) = maybe_argument {
        match maybe_argument.token {
            Some(CommandToken::Json(_)) => {
                // Argument is a JSON value which may be followed by a pipe or an equals
                command_parse_argument(parser, tokenizer).await?;

                parser.reduce(2, |cmd| {
                    let name        = cmd[0].token().unwrap().fragment.clone();
                    let argument    = cmd[1].node().unwrap().clone();

                    match argument {
                        CommandRequest::Command { argument, .. }    => CommandRequest::Command { command: CommandName(name), argument: argument },
                        _                                           => { unreachable!() }
                    }
                })?;

                // TODO: command can use pipe or an equals here
            }

            Some(CommandToken::Newline)     |
            Some(CommandToken::SemiColon)   => {
                // Command has no argument
                parser.accept_token()?;

                parser.reduce(2, |cmd| {
                    let name = cmd[0].token().unwrap().fragment.clone();
                    CommandRequest::Command { command: CommandName(name), argument: ParsedJson::Null }
                })?;
            }

            // TODO: command can have no arguments and be followed by a pipe

            _ => { return Err(maybe_argument.into()); }
        }
    } else {
        // No argument, so just a command
        parser.reduce(1, |cmd| {
            let name = cmd[0].token().unwrap().fragment.clone();
            CommandRequest::Command { command: CommandName(name), argument: ParsedJson::Null }
        })?;
    }

    Ok(())
}

///
/// Parses a 'Variable = <value>' assignment command
///
async fn command_parse_assignment<TStream>(parser: &mut Parser<TokenMatch<CommandToken>, CommandRequest>, tokenizer: &mut Tokenizer<CommandToken, TStream>) -> Result<(), CommandParseError>
where
    TStream: Send + Stream<Item=Vec<u8>>,
{
    // Lookahead should be 'command ='
    let command = parser.lookahead(0, tokenizer, |tokenizer| command_read_token(tokenizer).boxed()).await;

    // This should be a sanity check, as the lookahead should already be checked
    if command.as_ref().and_then(|cmd| cmd.token) != Some(CommandToken::Variable) {
        return Err(CommandParseError::UnexpectedToken(command.clone().and_then(|cmd| cmd.token), command.map(|cmd| cmd.fragment.clone()).unwrap_or(String::new())));
    }

    let equals = parser.lookahead(1, tokenizer, |tokenizer| command_read_token(tokenizer).boxed()).await;
    if equals.as_ref().and_then(|equals| equals.token) != Some(CommandToken::Equals) {
        return Err(CommandParseError::UnexpectedToken(equals.clone().and_then(|equals| equals.token), equals.map(|equals| equals.fragment.clone()).unwrap_or(String::new())));
    }

    // Accept the 'command =' tokens
    parser.accept_token()?;
    parser.accept_token()?;

    // Should be followed by another command
    command_parse(parser, tokenizer).await?;

    // Reduce as a variable assignment
    parser.reduce(3, |mut assignment| {
        let command     = assignment.pop().unwrap();
        let _equals     = assignment.pop().unwrap();
        let variable    = assignment.pop().unwrap();

        CommandRequest::Assign {
            variable:   VariableName(variable.to_token().unwrap().fragment),
            from:       Box::new(command.to_node().unwrap())
        }
    })?;

    // Matched
    Ok(())
}

///
/// Parses an argument to a command (resulting in a CommandRequest::Command with no name)
///
async fn command_parse_argument<TStream>(parser: &mut Parser<TokenMatch<CommandToken>, CommandRequest>, tokenizer: &mut Tokenizer<CommandToken, TStream>) -> Result<(), CommandParseError> 
where
    TStream: Send + Stream<Item=Vec<u8>>,
{
    // Create a JSON parser to read the following JSON value
    let mut json_parser = Parser::with_lookahead_from(parser);
    json_parse_value(&mut json_parser, tokenizer).await?;

    // Restore any lookahead to the original parser
    parser.take_lookahead_from(&mut json_parser);

    // Fetch the JSON value for the argument
    let json_value = json_parser.finish()?;

    // Add as a node to the current parser
    parser.reduce(0, |_| CommandRequest::Command { command: CommandName("".to_string()), argument: json_value.into() })?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

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
    fn match_command_immediately_on_newline() {
        let match_result = match_command("test\n", false);
        assert!(match_result == TokenMatchResult::Matches(CommandToken::Command, "test".chars().count()), "{:?}", match_result);
    }

    #[test]
    fn match_command_with_following_whitespace() {
        let match_result = match_command("test \n", false);
        assert!(match_result == TokenMatchResult::Matches(CommandToken::Command, "test".chars().count()), "{:?}", match_result);
    }

    #[test]
    fn tokenize_variable_1() {
        let variable        = ":variable";
        let mut tokenizer   = Tokenizer::new(stream::iter(variable.bytes()).ready_chunks(2));

        tokenizer.with_command_matchers();
        let variable_token = executor::block_on(async { command_read_token(&mut tokenizer).await });

        assert!(variable_token.is_some());
        let variable_token = variable_token.unwrap();

        assert!(variable_token.token == Some(CommandToken::Variable), "{:?}", variable_token);
        assert!(variable_token.fragment == ":variable", "{:?}", variable_token);
    }

    #[test]
    fn tokenize_variable_2() {
        let variable        = "$variable";
        let mut tokenizer   = Tokenizer::new(stream::iter(variable.bytes()).ready_chunks(2));

        tokenizer.with_command_matchers();
        let variable_token = executor::block_on(async { command_read_token(&mut tokenizer).await });

        assert!(variable_token.is_some());
        let variable_token = variable_token.unwrap();

        assert!(variable_token.token == Some(CommandToken::Variable), "{:?}", variable_token);
        assert!(variable_token.fragment == "$variable", "{:?}", variable_token);
    }

    #[test]
    fn tokenize_variable_3() {
        let variable        = ":variable = ";
        let mut tokenizer   = Tokenizer::new(stream::iter(variable.bytes()).ready_chunks(2));

        tokenizer.with_command_matchers();
        let variable_token = executor::block_on(async { command_read_token(&mut tokenizer).await });

        assert!(variable_token.is_some());
        let variable_token = variable_token.unwrap();

        assert!(variable_token.token == Some(CommandToken::Variable), "{:?}", variable_token);
        assert!(variable_token.fragment == ":variable", "{:?}", variable_token);
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

            assert!(result == CommandRequest::Command { command: CommandName("".to_string()), argument: json!{[1, 2, 3, 4]}.into() });
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

            assert!(result == CommandRequest::Command { command: CommandName("some::command".to_string()), argument: serde_json::Value::Null.into() });
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

            assert!(result == CommandRequest::Command { command: CommandName("some::command".to_string()), argument: json!{[1, 2, 3, 4]}.into() });
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

            assert!(result == CommandRequest::Command { command: CommandName("some::command".to_string()), argument: json!{[1, 2, 3, 4]}.into() });
        });
    }

    #[test]
    fn parse_command_with_json_argument_1() {
        let argument        = stream::iter("some::command { }\n".bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == CommandRequest::Command { command: CommandName("some::command".to_string()), argument: json!({}).into() });
        });
    }

    #[test]
    fn parse_command_with_json_argument_2() {
        let argument        = stream::iter("some::command { \"test\": \"test\" }\n".bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == CommandRequest::Command { command: CommandName("some::command".to_string()), argument: json!( { "test": "test" }).into() });
        });
    }

    #[test]
    fn parse_command_with_json_argument_3() {
        let argument        = stream::iter("some::command { \"test\": \"test\", \"number\": 4.5 }\n".bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(argument);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == CommandRequest::Command { command: CommandName("some::command".to_string()), argument: json!( { "test": "test", "number": 4.5 } ).into() });
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

            assert!(result == CommandRequest::Command { command: CommandName("some::command".to_string()), argument: json!{[1, 2, 3, 4]}.into() });
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
            assert!(result == CommandRequest::Command { command: CommandName("some::command".to_string()), argument: json!{[1, 2, 3, 4]}.into() });

            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();
            assert!(result == CommandRequest::Command { command: CommandName("another::command".to_string()), argument: serde_json::Value::Null.into() });

            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();
            assert!(result == CommandRequest::Command { command: CommandName("one_more".to_string()), argument: serde_json::Value::Null.into() });

            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();
            assert!(result == CommandRequest::Command { command: CommandName("and_another".to_string()), argument: json!{["Hello"]}.into() });
        });
    }

    #[test]
    fn parse_command_assignment() {
        let assignment      = stream::iter(r#":variable = some_command [ 1, 2, 3, 4 ]"#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(assignment);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == CommandRequest::Assign {
                variable:   VariableName(":variable".into()),
                from:       Box::new(CommandRequest::Command { command: CommandName("some_command".to_string()), argument: json!{[1, 2, 3, 4]}.into() })
            });
        });
    }

    #[test]
    fn parse_variable_command() {
        let variable        = stream::iter(r#"$variable"#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(variable);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == CommandRequest::Command { command: CommandName("$variable".to_string()), argument: serde_json::Value::Null.into() }, "{:?}", result);
        });
    }

    #[test]
    fn parse_raw_json_1() {
        let json            = stream::iter(r#"[ 1, 2, 3, 4 ]"#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(json);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == CommandRequest::RawJson { value: json!{[1, 2, 3, 4]}.into() }, "{:?}", result);
        });
    }

    #[test]
    fn parse_raw_json_2() {
        let json            = stream::iter(r#""string""#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(json);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == CommandRequest::RawJson { value: json!{"string"}.into() }, "{:?}", result);
        });
    }

    #[test]
    fn parse_raw_json_3() {
        let json            = stream::iter(r#"{ "test": 1 } "#.bytes()).ready_chunks(2);
        let mut tokenizer   = Tokenizer::new(json);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        executor::block_on(async {
            command_parse(&mut parser, &mut tokenizer).await.unwrap();
            let result = parser.finish().unwrap();

            assert!(result == CommandRequest::RawJson { value: json!{{"test": 1}}.into() }, "{:?}", result);
        });
    }
}
