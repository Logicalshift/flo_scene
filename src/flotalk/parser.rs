use super::program::*;
use super::location::*;
use super::parse_error::*;
use super::pushback_stream::*;

use flo_stream::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

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

/// True if the specified character is a letter that can be used in an identifier
#[inline]
fn is_letter(c: char) -> bool {
    c.is_alphabetic()
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
    async fn consume_comment(&mut self) -> Result<Option<ParserResult<String>>, ParserResult<TalkParseError>> {
        // In Smalltalk, comments start with a double-quote character '"'
        if self.peek().await != Some('"') { return Ok(None); }

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
                return Ok(Some(ParserResult { value: matched, location: comment_start.to(self.location()) }));
            }
        }

        Err(ParserResult { value: TalkParseError::UnclosedDoubleQuoteComment, location: comment_start.to(self.location()) })
    }

    ///
    /// Consumes any ignorable data
    ///
    async fn consume(&mut self) -> Result<(), ParserResult<TalkParseError>> {
        while let Some(c) = self.peek().await {
            if is_whitespace(c) {
                // Just consume whitespace
                self.next().await;
            } else if c == '"' {
                // Consume comments, and check for errors
                self.consume_comment().await?;
            } else {
                break;
            }
        }

        Ok(())
    }

    ///
    /// With the stream pointing to a "'" character, matches the following string
    ///
    async fn match_string(&mut self) -> Result<ParserResult<TalkLiteral>, ParserResult<TalkParseError>> {
        let start_location      = self.location();
        let mut string          = String::new();

        // Skip past the first "'"
        let first_quote = self.next().await;
        if first_quote != Some('\'') {
            return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location }); 
        }

        // Match the remaining characters in the string
        while let Some(next_chr) = self.next().await {
            if next_chr == '\'' {
                // Either '' (ie, a quote within the string), or the end of the string
                if self.peek().await == Some('\'') {
                    // Quote character
                    string.push(next_chr);
                    self.next().await;
                } else {
                    // End of string
                    return Ok(ParserResult { value: TalkLiteral::String(Arc::new(string)), location: start_location });
                }
            } else {
                // Part of the string
                string.push(next_chr);
            }
        }

        // Ran out of characters
        Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location })
    }

    ///
    /// After matching '#' and looking ahead to '(', finish matching the rest of the array
    ///
    async fn match_array(&mut self, start_location: TalkLocation) -> Result<ParserResult<TalkLiteral>, ParserResult<TalkParseError>> {
        todo!("Array")
    }

    ///
    /// Matches an identifier at the current position (can match 0 characters)
    ///
    async fn match_identifier(&mut self) -> ParserResult<Arc<String>> {
        let start_location = self.location();
        let mut identifier = String::new();

        // While there are characters to peek...
        while let Some(chr) = self.peek().await {
            // Stop if the peeked character isn't part of an identifier
            if !is_letter(chr) && !is_number(chr) {
                break;
            }

            // Consume this character
            identifier.push(chr);
            self.next().await;
        }

        // Whatever we matched is the identifier
        let identifier = Arc::new(identifier);
        ParserResult { value: identifier, location: start_location }
    }

    ///
    /// Matches a number at the current position (can match 0 characters)
    ///
    async fn match_number(&mut self) -> ParserResult<Arc<String>> {
        let start_location = self.location();
        let mut number = String::new();

        // First part of the number
        while let Some(chr) = self.peek().await {
            // Stop if the peeked character isn't part of a number
            if !is_number(chr) {
                break;
            }

            // Consume this character
            number.push(chr);
            self.next().await;
        }

        // Might be a float or a radix integer
        let follow_chr = self.peek().await;

        if follow_chr == Some('.') {
            // Floating point number
            number.push('.');
            self.next().await;

            while let Some(chr) = self.peek().await {
                // Stop if the peeked character isn't part of a number
                if !is_number(chr) {
                    break;
                }

                // Consume this character
                number.push(chr);
                self.next().await;
            }

            if self.peek().await == Some('e') {
                // Exponent
                number.push('e');
                self.next().await;

                while let Some(chr) = self.peek().await {
                    // Stop if the peeked character isn't part of a number
                    if !is_number(chr) {
                        break;
                    }

                    // Consume this character
                    number.push(chr);
                    self.next().await;
                }
            }

        } else if follow_chr == Some('r') {
            // Radix number
            number.push('r');
            self.next().await;

            while let Some(chr) = self.peek().await {
                // Stop if the peeked character isn't part of a radix number
                if !is_number(chr) && !is_letter(chr) {
                    break;
                }

                // Consume this character
                number.push(chr);
                self.next().await;
            }

        }

        // Whatever we matched is the number
        let number = Arc::new(number);
        ParserResult { value: number, location: start_location }
    }

    ///
    /// With the lookahead on the stream being a '#', match the following array or symbol
    ///
    async fn match_array_or_symbol(&mut self) -> Result<ParserResult<TalkLiteral>, ParserResult<TalkParseError>> {
        let start_location      = self.location();

        // Skip past the first "#"
        let hash = self.next().await;
        if hash != Some('#') {
            return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location }); 
        }

        // Decide what to do based on the next character
        let next_chr = self.peek().await;
        let next_chr = if let Some(chr) = next_chr { chr } else { return Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location }); };

        if next_chr == '(' {
            
            // Is an array
            self.match_array(start_location).await
        
        } else if next_chr == '\'' {
            
            // Is a hashed string
            let string          = self.match_string().await?;
            let string_value    = match string.value {
                TalkLiteral::String(value)  => Arc::clone(&value),
                _                           => Arc::new(String::new()),
            };

            Ok(ParserResult { value: TalkLiteral::Symbol(string_value), location: start_location })

        } else if is_letter(next_chr) {
            
            // Is a selector
            let mut identifier = self.match_identifier().await.value;
            if self.peek().await == Some(':') {
                // Keyword selector
                Arc::make_mut(&mut identifier).push(':');
                self.next().await;
            }

            Ok(ParserResult { value: TalkLiteral::Selector(identifier), location: start_location })

        } else {

            // Not a valid '#' sequence
            Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location })
        }
    }

    ///
    /// With the stream at the first character in a literal, matches and consumes that literal
    ///
    async fn match_literal(&mut self) -> Result<Option<ParserResult<TalkLiteral>>, ParserResult<TalkParseError>> {
        let start_location      = self.location();

        // Read the first character of the literal (error if we're at the end of the file)
        let chr = self.peek().await;
        let chr = if let Some(chr) = chr { 
            chr
        } else { 
            return Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location }); 
        };

        // Match the literal based on the first character
        if chr == '$' {

            // Character
            let _chr = self.next().await.unwrap();
            debug_assert!(_chr == '$');

            let chr = self.next().await;
            if let Some(chr) = chr {
                Ok(Some(ParserResult { value: TalkLiteral::Character(chr), location: start_location.to(self.location()) }))
            } else {
                Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location })
            }

        } else if chr == '\'' {

            // String
            Ok(Some(self.match_string().await?))

        } else if chr == '#' {

            // Array if #( or symbol if #' or #<alphanum>
            Ok(Some(self.match_array_or_symbol().await?))

        } else if is_number(chr) {

            // Number
            let number = self.match_number().await;
            Ok(Some(ParserResult { value: TalkLiteral::Number(number.value), location: start_location.to(self.location()) }))

        } else if chr == '-' {

            // Might be number, depends on next character
            self.next().await;

            if is_number(self.peek().await.unwrap_or(' ')) {
                let mut number = self.match_number().await;

                Arc::make_mut(&mut number.value).insert(0, '-');

                Ok(Some(ParserResult { value: TalkLiteral::Number(number.value), location: start_location.to(self.location()) }))
            } else {
                Err(ParserResult { value: TalkParseError::UnexpectedCharacter('-'), location: start_location })
            }

        } else {

            // Unexpected character (ie, this is not a literal)
            return Ok(None);

        }
    }

    ///
    /// Matches a variable declaration (when the next character on the stream is the initial '|'
    ///
    async fn match_variable_declaration(&mut self) -> Result<ParserResult<Vec<Arc<String>>>, ParserResult<TalkParseError>> {
        let start_location  = self.location();
        let mut variables   = vec![];

        // Opening '|' and whitespace
        let initial_pipe = self.next().await;
        if initial_pipe != Some('|') { return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location }); }

        self.consume().await?;

        // Variables are identiifers
        loop {
            let next_chr = self.peek().await;
            let next_chr = if let Some(next_chr) = next_chr { next_chr } else { return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location.to(self.location()) }); };

            if is_letter(next_chr) {
                // Identifier
                let identifier = self.match_identifier().await;
                variables.push(identifier.value);

                self.consume().await?;
            } else if next_chr == '|' {
                // End of variable list
                return Ok(ParserResult { value: variables, location: start_location.to(self.location()) });
            } else {
                // Unexpected character
                return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location.to(self.location()) });
            }
        }
    }

    ///
    /// When the next character is the opening '[' of a block, matches the block's contents
    ///
    async fn match_block(&mut self) -> Result<ParserResult<(Vec<Arc<String>>, Vec<TalkExpression>)>, ParserResult<TalkParseError>> {
        let start_location  = self.location();

        // Consume the '['
        let opening_bracket = self.next().await;
        if opening_bracket != Some('[') {
            return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location });
        }

        // An initial comment can become the documentation for this block
        let mut initial_comment = None;
        self.consume_whitespace().await;

        if let Some(comment) = self.consume_comment().await? {
            initial_comment = Some(Arc::new(comment.value));
        }

        // Parameters are next, of the form `... :a :b | ...`
        self.consume().await?;

        let mut arguments = vec![];
        if self.peek().await == Some(':') {
            loop {
                let next_chr = self.next().await;
                let next_chr = if let Some(next_chr) = next_chr { next_chr } else { return Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location.to(self.location()) }); };

                // Pipe character ends the parameters
                if next_chr == '|' {
                    break;
                }

                // ':' is the expected character to start a parameter
                if next_chr != ':' {
                    return Err(ParserResult { value: TalkParseError::UnexpectedCharacter(next_chr), location: start_location.to(self.location()) });
                }

                // Should be followed by an identiifer
                let identifier = self.match_identifier().await.value;
                if identifier.len() == 0 {
                    let bad_chr = self.peek().await.unwrap_or(' ');
                    return Err(ParserResult { value: TalkParseError::UnexpectedCharacter(bad_chr), location: start_location.to(self.location()) });
                }

                // Add to the arguments
                arguments.push(identifier);

                // Eat up any comments/whitespace/etc between identifiers
                self.consume().await?;
            }

            // We treat the place after the '|' as another opportunity for a block comment
            if initial_comment.is_none() {
                self.consume_whitespace().await;

                if let Some(comment) = self.consume_comment().await? {
                    initial_comment = Some(Arc::new(comment.value));
                }

                self.consume().await?;
            }
        }

        // Rest of the block is expressions until we hit the ']'
        let mut expressions = vec![];
        loop {
            // Eat up whitespace, comments, etc
            self.consume().await?;

            // Peek at the next character
            let next_chr = self.peek().await;
            let next_chr = if let Some(next_chr) = next_chr { next_chr } else { return Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location.to(self.location()) }); };

            if next_chr == ']' {
                // Block finished
                self.next().await;

                return Ok(ParserResult { value: (arguments, expressions), location: start_location.to(self.location()) });
            }

            // Read the next expression
            let expression = self.match_expression().await?;
            let expression = if let Some(expression) = expression { expression } else { return Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location.to(self.location()) }); };

            // Add to the expressions making up this block
            expressions.push(expression.value);
        }
    }

    ///
    /// Matches a 'primary' at the current position (None if the current position is not a primary)
    ///
    async fn match_primary(&mut self) -> Result<Option<ParserResult<TalkExpression>>, ParserResult<TalkParseError>> {
        let start_location      = self.location();

        let chr             = self.peek().await;
        let mut chr         = if let Some(chr) = chr { chr } else { return Ok(None); };

        if chr == '(' {

            // Nested expression
            self.next().await;
            let mut expr = self.match_expression().await?;

            let mut expr = match expr {
                Some(expr)  => expr,
                None        => { return Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location.to(self.location()) }); }
            };

            // Closing bracket
            self.consume().await?;
            let closing_bracket = self.next().await;

            if closing_bracket != Some(')') {
                Err(ParserResult { value: TalkParseError::MissingCloseBracket, location: start_location.to(self.location()) })
            } else {
                Ok(Some(expr))
            }

        } else if chr == '[' {

            // Block
            let block                       = self.match_block().await?;
            let (arguments, expressions)    = block.value;

            Ok(Some(ParserResult { value: TalkExpression::Block(arguments, expressions), location: start_location.to(self.location()) }))

        } else if chr == '|' {

            // Variable declaration
            // (In SmallTalk-80, these are only allowed at the start of blocks, but we allow them anywhere and treat them as expressions)
            let variables = self.match_variable_declaration().await?;

            Ok(Some(ParserResult { value: TalkExpression::VariableDeclaration(variables.value), location: start_location.to(self.location()) }))

        } else if is_letter(chr) {

            // Identifier
            let identifier = self.match_identifier().await;

            Ok(Some(ParserResult { value: TalkExpression::Identifier(identifier.value), location: start_location.to(self.location()) }))

        } else {

            // Should be a literal
            let literal = self.match_literal().await?;
            match literal {
                Some(literal)   => Ok(Some(ParserResult { value: TalkExpression::Literal(literal.value), location: start_location.to(self.location()) })),
                None            => Ok(None),
            }
        }
    }

    ///
    /// Matches and returns the next expression on this stream (skipping whitespace and comments). Returns None if there are no more
    /// expressions (end of stream).
    ///
    fn match_expression<'a>(&'a mut self) -> BoxFuture<'a, Result<Option<ParserResult<TalkExpression>>, ParserResult<TalkParseError>>> {
        async move {
            // Eat up as much whitespace as possible
            self.consume_whitespace().await;

            // This point counts as the start of the expression
            let start_location      = self.location();
            let mut initial_comment = None;

            loop {
                if let Some(new_comment) = self.consume_comment().await? {
                    // Amend the initial comment
                    initial_comment = match initial_comment {
                        None                    => Some(Arc::new(new_comment.value)),
                        Some(mut old_comment)   => {
                            Arc::make_mut(&mut old_comment).push_str(&new_comment.value);
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
            let chr             = self.peek().await;
            let mut chr         = if let Some(chr) = chr { chr } else { return Ok(None); };
            let mut is_return   = false;

            // '.' is used to end expressions, if it's here by itself, return an empty expression
            if chr == '.' {
                self.next().await;
                return Ok(Some(ParserResult { value: TalkExpression::Empty, location: start_location.to(self.location()) }));
            }

            // Expressions starting '^' are return values (traditionally only allowed at the end of blocks)
            if chr == '^' {
                is_return = true;

                self.next().await;
                chr = if let Some(chr) = self.peek().await { chr } else { return Ok(None) };
            }

            // Fetch the 'primary' part of the expression
            let primary = self.match_primary().await?;
            let primary = if let Some(primary) = primary { primary } else { return Ok(None) };

            // TODO: variable declaration is an expression by itself, can't send messages to it

            // TODO: `identifier ::=` is an assignment

            // TODO: Following values indicate any messages to send

            // Return the result
            Ok(Some(primary))
        }.boxed()
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
        loop {
            let expression = input_stream.match_expression().await;

            match expression {
                Err(err)        => yield_value(Err(err)).await,
                Ok(Some(expr))  => yield_value(Ok(expr)).await,
                Ok(None)        => {
                    if let Some(err_char) = input_stream.peek().await {
                        // Unexpected character
                        let location = input_stream.location();
                        yield_value(Err(ParserResult { value: TalkParseError::UnexpectedCharacter(err_char), location: location })).await;
                        break;
                    } else {
                        // End of stream
                        break;
                    }
                }
            }
        }
    })
}
