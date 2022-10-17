use super::program::*;
use super::location::*;
use super::parse_error::*;
use super::pushback_stream::*;

use flo_stream::*;

use futures::prelude::*;

use std::sync::*;

///
/// A parser result
///
#[derive(Clone, PartialEq, Debug)]
pub struct ParserResult<TResult> {
    /// The parsed value
    pub value: TResult,

    /// The location where this result was generated from
    pub location: TalkLocation,

    /// The text that was matched for this result
    pub matched: Arc<String>,
}

/// True if the specified character is a whitespace character
#[inline]
fn is_whitespace(c: char) -> bool {
    c.is_whitespace()
}

/// True if the specified character is a number character
#[inline]
fn is_number(c: char) -> bool {
    c.is_numeric()
}

impl<TStream> PushBackStream<TStream>
where
    TStream: Unpin + Send + Stream<Item=char>
{
    ///
    /// Consumes as much whitespace as possible
    ///
    async fn consume_whitespace(&mut self) {
        // Read characters until we receive a non-whitespace character, then push it back
        while let Some(c) = self.next().await {
            if !is_whitespace(c) {
                self.pushback(c);
                break;
            }
        }
    }

    ///
    /// Consumes a comment, if one exists at the present location, returning as an empty parser result
    ///
    async fn consume_comment(&mut self) -> Option<Result<ParserResult<()>, ParserResult<TalkParseError>>> {
        // In Smalltalk, comments start with a double-quote character '"'
        if self.peek().await != Some('"') { return None; }

        // Remember where the comment starts
        let comment_start   = self.location();
        let mut matched     = String::new();

        // Consume the first '"'
        let first_quote = self.next().await;
        debug_assert!(first_quote == Some('"'));
        matched.push(first_quote.unwrap());

        // Read until the closing '"' (or the end of the stream)
        while let Some(chr) = self.next().await {
            matched.push(chr);

            if chr == '"' {
                // End of comment
                return Some(Ok(ParserResult { value: (), location: comment_start.to(self.location()), matched: Arc::new(matched) }));
            }
        }

        Some(Err(ParserResult { value: TalkParseError::UnclosedDoubleQuoteComment, location: comment_start.to(self.location()), matched: Arc::new(matched) }))
    }

    ///
    /// With the stream pointing to a "'" character, matches the following string
    ///
    async fn match_string(&mut self) -> Result<ParserResult<TalkLiteral>, ParserResult<TalkParseError>> {
        let start_location      = self.location();
        let mut matched         = String::new();
        let mut string          = String::new();

        // Skip past the first "'"
        let first_quote = self.next().await;
        if first_quote != Some('\'') {
            return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location, matched: Arc::new(matched) }); 
        }

        matched.push('\'');

        // Match the remaining characters in the string
        while let Some(next_chr) = self.next().await {
            // Add to the matched characters
            matched.push(next_chr);

            if next_chr == '\'' {
                // Either '' (ie, a quote within the string), or the end of the string
                if self.peek().await == Some('\'') {
                    // Quote character
                    string.push(next_chr);
                    self.next().await;
                } else {
                    // End of string
                    return Ok(ParserResult { value: TalkLiteral::String(Arc::new(string)), location: start_location, matched: Arc::new(matched) });
                }
            } else {
                // Part of the string
                string.push(next_chr);
            }
        }

        // Ran out of characters
        Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location, matched: Arc::new(matched) })
    }

    ///
    /// With the stream at the first character in a literal, matches and consumes that literal
    ///
    async fn match_literal(&mut self) -> Result<ParserResult<TalkLiteral>, ParserResult<TalkParseError>> {
        let start_location      = self.location();
        let mut matched         = String::new();

        // Read the first character of the literal (error if we're at the end of the file)
        let chr = self.peek().await;
        let chr = if let Some(chr) = chr { 
            chr
        } else { 
            return Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location, matched: Arc::new(matched) }); 
        };

        // Match the literal basedon the first character
        if chr == '[' {

            // Block
            todo!("Block")

        } else if chr == '$' {

            // Character
            let chr = self.next().await.unwrap();
            debug_assert!(chr == '$');
            matched.push(chr);

            let chr = self.next().await;
            if let Some(chr) = chr {
                Ok(ParserResult { value: TalkLiteral::Character(chr), location: start_location.to(self.location()), matched: Arc::new(matched) })
            } else {
                Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location, matched: Arc::new(matched) })
            }

        } else if chr == '\'' {

            // String
            self.match_string().await

        } else if chr == '#' {

            // Array if #( or symbol if #' or #<alphanum>
            todo!("Array_or_symbol")

        } else if is_number(chr) {

            // Number
            todo!("Numbers")

        } else if chr == '-' {

            // Might be number, depends on next character
            todo!("Negative numbers")

        } else {

            // Unexpected character
            Err(ParserResult { value: TalkParseError::UnexpectedCharacter(chr), location: start_location, matched: Arc::new(matched) })

        }
    }

    ///
    /// Matches and returns the next expression on this stream (skipping whitespace and comments). Returns None if there are no more
    /// expressions.
    ///
    async fn match_expression(&mut self) -> Option<Result<ParserResult<TalkExpression>, ParserResult<TalkParseError>>> {
        // Eat up as much whitespace as possible
        self.consume_whitespace().await;

        // This point counts as the start of the expression
        let start_location      = self.location();
        let mut initial_comment = None;

        loop {
            if let Some(new_comment) = self.consume_comment().await {
                let new_comment = match new_comment {
                    Ok(comment) => comment,
                    Err(err)    => return Some(Err(err))
                };

                // Amend the initial comment
                initial_comment = match initial_comment {
                    None                    => Some(new_comment.matched),
                    Some(mut old_comment)   => {
                        Arc::make_mut(&mut old_comment).push_str(&*new_comment.matched);
                        Some(old_comment)
                    }
                };

                // Consume whitespace following the comment
                self.consume_whitespace().await;
            } else {
                // No longer at a comment
                break;
            }
        }

        // Decide what type of expression is following
        let chr = self.peek().await;
        let chr = if let Some(chr) = chr { chr } else { return None; };

        let expr = if chr == '.' {
            // End of expression/empty expression
            self.next().await;
            todo!("Empty expression")
        } else if chr == '(' {
            // Nested expression
            todo!("Brackets")
        } else if chr == '|' {
            // Variable declaration
            todo!("Variable declaration")
        } else if chr == '^' {
            // Return statement
            todo!("Return statement")
        } else {
            // Should be a literal
            let literal = self.match_literal().await;
            match literal {
                Ok(literal) => ParserResult { value: TalkExpression::Literal(literal.value), location: start_location, matched: literal.matched },
                Err(err)    => { return Some(Err(err)); }
            }
        };

        Some(Ok(expr))
    }
}

///
/// Parses a flotalk expression stream
///
pub fn parse_flotalk_expression<'a>(input_stream: impl 'a + Unpin + Send + Stream<Item=char>) -> impl 'a + Send + Stream<Item=Result<ParserResult<TalkExpression>, ParserResult<TalkParseError>>> {
    let input_stream = PushBackStream::new(input_stream);

    // Use a generator stream to output the values
    generator_stream(move |yield_value| async move {
        let mut input_stream = input_stream;

        // Match as many expressions as possible
        while let Some(expression) = input_stream.match_expression().await {
            yield_value(expression).await;
        }
    })
}
