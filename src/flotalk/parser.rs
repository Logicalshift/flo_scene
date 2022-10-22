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

/// True if the specified character can be part of a binary selector
#[inline]
fn is_binary_character(c: char) -> bool {
    match c {
        '+' | '-' | '/' | '*' | '!' | '%' | '&' | ','| '<' | '=' | '>' | '?' | '@' | '\\' | '~' | '|' => true,
        _ => false,
    }
}

impl<TStream> PushBackStream<TStream>
where
    TStream: Unpin + Send + Stream<Item=char>
{
    ///
    /// Consumes as much whitespace as possible
    ///
    /// `<whitespace> ::= <whitespace_character>*`
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
    /// `<comment> ::= '"' <anything-but-double-quote>* '"'`
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
    /// `<consume> ::= [ <comment> | <whitespace> ]*`
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
    /// `<string> ::= '\'' <anything-but-single-quote>* '\''`
    ///
    async fn match_string(&mut self) -> Result<ParserResult<TalkLiteral>, ParserResult<TalkParseError>> {
        let start_location      = self.location();
        let mut string          = String::new();

        // Skip past the first "'"
        let first_quote = self.next().await;
        if first_quote != Some('\'') {
            debug_assert!(false, "Expected '\''");
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
    /// `<array-definition> ::= '(' <literal>+ ')' `
    ///
    fn match_array<'a>(&'a mut self, start_location: TalkLocation) -> BoxFuture<'a, Result<ParserResult<TalkLiteral>, ParserResult<TalkParseError>>> {
        async move {
            // Should be a bracket waiting to be read
            let opening_bracket = self.next().await;
            if opening_bracket != Some('(') {
                debug_assert!(false, "Expected '('");
                return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location }); 
            }

            // Array is a series of literals
            let mut literals = vec![];
            self.consume().await?;
            while let Some(literal) = self.match_literal().await? {
                literals.push(literal.value);
                self.consume().await?;
            }

            // Array should finish with a closing bracket
            let closing_bracket = self.next().await;
            if closing_bracket != Some(')') {
                return Err(ParserResult { value: TalkParseError::UnexpectedCharacter(closing_bracket.unwrap_or(' ')), location: self.location() }); 
            }

            return Ok(ParserResult { value: TalkLiteral::Array(literals), location: start_location.to(self.location()) })
        }.boxed()
    }

    ///
    /// Matches an identifier at the current position (can match 0 characters)
    ///
    /// `<identifier> ::= <letter> [ <letter> | <numeric> ]*`
    /// `<keyword>    ::= <identifier> ':'`
    ///
    async fn match_identifier_or_keyword(&mut self) -> ParserResult<TalkIdentifierOrKeyword> {
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

        if self.peek().await == Some(':') {
            // Matched `foo:`, ie, a keyword
            self.next().await;
            identifier.push(':');

            let keyword = Arc::new(identifier);
            ParserResult { value: TalkIdentifierOrKeyword::Keyword(keyword), location: start_location.to(self.location()) }
        } else {
            // Whatever we matched is the identifier
            let identifier = Arc::new(identifier);
            ParserResult { value: TalkIdentifierOrKeyword::Identifier(identifier), location: start_location.to(self.location()) }
        }
    }

    ///
    /// Matches an identifier or returns an error if the value is a keyword
    ///
    async fn match_identifier(&mut self) -> Result<ParserResult<Arc<String>>, ParserResult<TalkParseError>> {
        let maybe_identifier = self.match_identifier_or_keyword().await;

        match maybe_identifier.value {
            TalkIdentifierOrKeyword::Identifier(identifier) => Ok(ParserResult { value: identifier, location: maybe_identifier.location }),
            TalkIdentifierOrKeyword::Keyword(_)             => Err(ParserResult { value: TalkParseError::KeywordNotValidHere, location: maybe_identifier.location }),
        }
    }

    ///
    /// Matches a number at the current position (can match 0 characters)
    ///
    /// `<number> ::= <numeric>*`
    /// `<number> ::= <numeric>* 'r' [ <numeric> | <letter> ]*`
    /// `<number> ::= <numeric>* '.' <numeric>*`
    /// `<number> ::= <numeric>* '.' <numeric>* 'e' <numeric>*`
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
    /// `<symbol> ::= '#' <identifier>`
    /// `<symbol> ::= '#' <keyword>`
    /// `<symbol> ::= '#' <string>`
    /// `<array>  ::= '#' '(' <array-definition>`
    ///
    async fn match_array_or_symbol(&mut self) -> Result<ParserResult<TalkLiteral>, ParserResult<TalkParseError>> {
        let start_location      = self.location();

        // Skip past the first "#"
        let hash = self.next().await;
        if hash != Some('#') {
            debug_assert!(false, "Expected '#'");
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
            let identifier = self.match_identifier_or_keyword().await.value;
            let identifier = match identifier {
                TalkIdentifierOrKeyword::Identifier(identifier) => identifier,
                TalkIdentifierOrKeyword::Keyword(keyword)       => keyword,
            };

            Ok(ParserResult { value: TalkLiteral::Selector(identifier), location: start_location })

        } else {

            // Not a valid '#' sequence
            Err(ParserResult { value: TalkParseError::ExpectedMoreCharacters, location: start_location })
        }
    }

    ///
    /// With the stream at the first character in a literal, matches and consumes that literal, returning None if the stream is not at a literal
    ///
    /// `<literal>   ::= <character> | <string> | <array> | <symbol> | <number> | '-' <number>`
    /// `<character> ::= '$' <any>`
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
    /// `<variable-declaration> ::= '|' <identifier>* '|'`
    ///
    async fn match_variable_declaration(&mut self) -> Result<ParserResult<Vec<Arc<String>>>, ParserResult<TalkParseError>> {
        let start_location  = self.location();
        let mut variables   = vec![];

        // Opening '|' and whitespace
        let initial_pipe = self.next().await;
        if initial_pipe != Some('|') { 
            debug_assert!(false, "Expected '|'");
            return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location });
        }

        self.consume().await?;

        // Variables are identiifers
        loop {
            let next_chr = self.peek().await;
            let next_chr = if let Some(next_chr) = next_chr { next_chr } else { return Err(ParserResult { value: TalkParseError::InconsistentState, location: start_location.to(self.location()) }); };

            if is_letter(next_chr) {
                // Identifier
                let identifier = self.match_identifier().await?;
                variables.push(identifier.value);

                self.consume().await?;
            } else if next_chr == '|' {
                // End of variable list
                self.next().await;
                return Ok(ParserResult { value: variables, location: start_location.to(self.location()) });
            } else {
                // Unexpected character
                return Err(ParserResult { value: TalkParseError::UnexpectedCharacter(next_chr), location: start_location.to(self.location()) });
            }
        }
    }

    ///
    /// When the next character is the opening '[' of a block, matches the block's contents
    ///
    /// `<block> ::= '[' <expression>* ']'`
    /// `<block> ::= '[' [ ':' <identifier> ]* '|' <expression>* ']'`
    ///
    async fn match_block(&mut self) -> Result<ParserResult<(Vec<Arc<String>>, Vec<TalkExpression>)>, ParserResult<TalkParseError>> {
        let start_location  = self.location();

        // Consume the '['
        let opening_bracket = self.next().await;
        if opening_bracket != Some('[') {
            debug_assert!(false, "Expected '['");
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
                let identifier = self.match_identifier().await?.value;
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
    /// Matches a unary message, if one is at the current position in the stream
    ///
    /// `<unary-message> ::= <identifier>`
    ///
    async fn match_unary_message(&mut self) -> Option<TalkArgument> {
        let chr = self.peek().await?;
        
        if is_letter(chr) {
            // Either an identifier or a keyword
            let identifier_or_keyword = self.match_identifier_or_keyword().await;

            match identifier_or_keyword.value {
                TalkIdentifierOrKeyword::Keyword(keyword) => {
                    // Push the entire keyword back
                    for chr in keyword.chars().rev() {
                        self.pushback(chr)
                    }

                    // Not a unary message
                    None
                }

                TalkIdentifierOrKeyword::Identifier(identifier) => {
                    Some(TalkArgument { name: identifier, value: None })
                }
            }
        } else {
            // Not an identifier
            None
        }
    }

    ///
    /// Matches a binary message, if one is next on the stream
    ///
    /// `<binary-message>  ::= <binary-selector> <binary-argument>`
    /// `<binary-argument> ::= <primary> <unary-message>*`
    ///
    async fn match_binary_message(&mut self) -> Result<Option<TalkArgument>, ParserResult<TalkParseError>> {
        let start_location = self.location();

        let chr = self.peek().await;
        let chr = if let Some(chr) = chr { chr } else { return Ok(None); };

        // The binary selector is one or more binary characters
        if !is_binary_character(chr) { return Ok(None); }

        self.next().await;
        let mut binary_selector = String::new();
        binary_selector.push(chr);

        while let Some(chr) = self.peek().await {
            if !is_binary_character(chr) { break; }

            self.next().await;
            binary_selector.push(chr);
        }

        // Can be whitespace between the binary selector and the arguments
        self.consume().await?;

        // Argument must be a primary, followed by any number of unary messages
        let primary = self.match_primary().await?;
        let primary = if let Some(primary) = primary { primary } else { return Err(ParserResult { value: TalkParseError::MissingRValue, location: start_location.to(self.location()) }); };

        // After the primary comes a list of 0 or more unary messages
        self.consume().await?;

        let mut unary_messages = vec![];
        while let Some(unary_message) = self.match_unary_message().await {
            unary_messages.push(unary_message);

            self.consume().await?;
        }

        // The unary messages are applied to the 'primary'
        let mut message = primary.value;

        if unary_messages.len() > 0 {
            // Messages apply to the previous result
            for unary_message in unary_messages.into_iter() {
                message = TalkExpression::SendMessage(Box::new(message), vec![unary_message]);
            }
        }

        Ok(Some(TalkArgument { name: Arc::new(binary_selector), value: Some(message) }))
    }

    ///
    /// Matches a keyword message argument, if one is on the stream
    ///
    /// `<keyword-message-argument> ::= <keyword> <keyword-argument>`
    /// `<keyword-argument>         ::= <primary> <unary-message>* <binary-message>*`
    ///
    async fn match_keyword_message_argument(&mut self) -> Result<Option<ParserResult<TalkArgument>>, ParserResult<TalkParseError>> {
        let start_location = self.location();

        // Match the next keyword
        let maybe_keyword = self.match_identifier_or_keyword().await;

        let keyword = match maybe_keyword.value {
            TalkIdentifierOrKeyword::Keyword(keyword)       => keyword,
            TalkIdentifierOrKeyword::Identifier(identifier) => {
                // Push the entire identifier back
                for chr in identifier.chars().rev() {
                    self.pushback(chr)
                }

                // Not a keyword message
                return Ok(None);
            }
        };

        // Followed by the keyword arguments
        self.consume().await?;

        let primary = self.match_primary().await?;
        let primary = if let Some(primary) = primary { primary } else { return Err(ParserResult { value: TalkParseError::MissingMessageArgument, location: start_location.to(self.location()) }) };

        // Any number of unary messages, followed by any number of binary messages
        let mut unary_messages = vec![];
        self.consume().await?;

        while let Some(unary_message) = self.match_unary_message().await {
            unary_messages.push(unary_message);
            self.consume().await?;
        }

        let mut binary_messages = vec![];
        self.consume().await?;

        while let Some(binary_message) = self.match_binary_message().await? {
            binary_messages.push(binary_message);
            self.consume().await?;
        }

        // Combine into a message
        let mut message = primary.value;

        if unary_messages.len() > 0 {
            // Messages apply to the previous result
            for unary_message in unary_messages.into_iter() {
                message = TalkExpression::SendMessage(Box::new(message), vec![unary_message]);
            }
        }

        if binary_messages.len() > 0 {
            for binary_argument in binary_messages {
                message = TalkExpression::SendMessage(Box::new(message), vec![binary_argument]);
            }
        }

        Ok(Some(ParserResult { value: TalkArgument { name: keyword, value: Some(message) }, location: start_location.to(self.location()) }))
    }

    ///
    /// Matches the 'messages' portion of an expression (or None if there are no messages)
    ///
    /// Return value is 'unary messages', 'binary messages', 'keyword arguments'. Unary and binary messages are both applied to their output, the keyword arguments
    /// are a single block of arguments applied to the output of the unary and binary messages.
    ///
    /// `<messages>         ::= <unary-message>+ <binary-message>*`
    /// `<messages>         ::= <unary-message>+ <binary-message>* <keyword-message>`
    /// `<messages>         ::= <binary-message>+`
    /// `<messages>         ::= <binary-message>+ <keyword-message>`
    /// `<messages>         ::= <keyword-message>`
    /// `<unary-message>    ::= <identifier>`
    /// `<binary-message>   ::= <binary-selector> <binary-argument>`
    /// `<binary-argument>  ::= <primary> <unary-message>*`
    /// `<keyword-message>  ::= ( <keyword> <keyword-argument> )+`
    /// `<keyword-argument> ::= <primary> <unary-message>* <binary-message>*`
    ///
    async fn match_messages(&mut self) -> Result<Option<ParserResult<(Vec<TalkArgument>, Vec<TalkArgument>, Vec<TalkArgument>)>>, ParserResult<TalkParseError>> {
        let start_location = self.location();

        // Any number of unary messages
        let mut unary_messages = vec![];
        self.consume().await?;

        while let Some(unary_message) = self.match_unary_message().await {
            unary_messages.push(unary_message);
            self.consume().await?;
        }

        // Any number of binary messages
        let mut binary_messages = vec![];

        while let Some(binary_message) = self.match_binary_message().await? {
            binary_messages.push(binary_message);
            self.consume().await?;
        }

        // Any number of keyword messages
        let mut keyword_arguments = vec![];

        while let Some(keyword_message) = self.match_keyword_message_argument().await? {
            keyword_arguments.push(keyword_message.value);
            self.consume().await?;
        }

        if unary_messages.len() == 0 && binary_messages.len() == 0 && keyword_arguments.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(ParserResult { value: (unary_messages, binary_messages, keyword_arguments), location: start_location.to(self.location()) }))
        }
    }

    ///
    /// Applies the specified message arguments to an expression
    ///
    fn apply_messages(&self, expr: TalkExpression, (unary_messages, binary_messages, keyword_arguments): (Vec<TalkArgument>, Vec<TalkArgument>, Vec<TalkArgument>)) -> TalkExpression {
        let mut expr = expr;

        if unary_messages.len() > 0 {
            // Messages apply to the previous result
            for unary_message in unary_messages.into_iter() {
                expr = TalkExpression::SendMessage(Box::new(expr), vec![unary_message]);
            }
        }

        if binary_messages.len() > 0 {
            for binary_argument in binary_messages {
                expr = TalkExpression::SendMessage(Box::new(expr), vec![binary_argument]);
            }
        }

        if keyword_arguments.len() > 0 {
            expr = TalkExpression::SendMessage(Box::new(expr), keyword_arguments);
        }

        expr
    }

    ///
    /// Matches a 'primary' at the current position (None if the current position is not a primary)
    ///
    /// `<primary> ::= '(' <expression> `)` | <block> | <variable-declaration> | <identifier> | <literal>`
    ///
    async fn match_primary(&mut self) -> Result<Option<ParserResult<TalkExpression>>, ParserResult<TalkParseError>> {
        let start_location  = self.location();

        let chr = self.peek().await;
        let chr = if let Some(chr) = chr { chr } else { return Ok(None); };

        if chr == '(' {

            // Nested expression
            self.next().await;
            let expr = self.match_expression().await?;

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
            let identifier = self.match_identifier().await?;

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
    /// `<expression> ::= '.'`
    /// `<expression> ::= <primary> '.'?`
    /// `<expression> ::= <primary> <messages> '.'?`
    /// `<expression> ::= `^` <expression> '.'?`
    /// `<expression> ::= <identifier> ':' ':' '=' <expression> '.'?`
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
                self.consume().await?;
                chr = if let Some(chr) = self.peek().await { chr } else { return Ok(None) };
            }

            // Fetch the 'primary' part of the expression
            let primary = self.match_primary().await?;
            let primary = if let Some(primary) = primary { primary } else { return Ok(None) };

            // Variable declaration is an expression by itself, can't send messages to it
            if let TalkExpression::VariableDeclaration(_) = primary.value {
                return Ok(Some(primary));
            }

            // `identifier ::=` is an assignment
            if let TalkExpression::Identifier(identifier) = &primary.value {
                let identifier = Arc::clone(identifier);

                // We only allow whitespace between the identifier and the '::=' and not comments, to allow for documentation comments elsewhere
                self.consume_whitespace().await;

                if self.peek().await == Some(':') {
                    // Look for the rest of the '::='
                    self.next().await;
                    if self.next().await == Some('=') {
                        // Is an assignment
                        let assignment_expr = self.match_expression().await?.unwrap_or(ParserResult { value: TalkExpression::Empty, location: self.location() });

                        // Can't send any more messages to an assignment
                        return Ok(Some(ParserResult { value: TalkExpression::Assignment(identifier, Box::new(assignment_expr.value)), location: start_location.to(self.location()) }));
                    } else {
                        // Looked like an assignment but turned out to be something else
                        return Err(ParserResult { value: TalkParseError::UnexpectedCharacter(self.peek().await.unwrap_or(' ')), location: self.location() });
                    }
                }
            }

            // Following values indicate any messages to send
            let mut expression = primary;

            let messages = self.match_messages().await?;
            if let Some(messages) = messages {
                let mut messages = vec![messages];

                // Read any cascading messages that might follow the expression
                loop {
                    self.consume_whitespace().await;

                    if self.peek().await != Some(';') {
                        // No more messages
                        break;
                    }

                    self.next().await;
                    self.consume().await?;

                    // Add as a cascading message
                    if let Some(cascading_message) = self.match_messages().await? {
                        messages.push(cascading_message);
                    } else {
                        return Err(ParserResult { value: TalkParseError::MissingCascadingMessage, location: self.location() });
                    }
                }

                // Apply the messages to the result
                if messages.len() == 1 {
                    // Just a single message
                    expression.value = self.apply_messages(expression.value, messages.pop().unwrap().value);
                } else {
                    // Cascaded messages are applied to the same primary value
                    let cascaded        = messages.into_iter().map(|msg| self.apply_messages(TalkExpression::CascadePrimaryResult, msg.value));
                    expression.value    = TalkExpression::CascadeFrom(Box::new(expression.value), cascaded.collect());
                }

                expression.location = expression.location.to(self.location());
            }

            // Apply the 'return' value
            if is_return {
                expression.value = TalkExpression::Return(Box::new(expression.value));
            }

            // Return the result
            Ok(Some(expression))
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

            // '.' can be used to separate expressions
            input_stream.consume_whitespace().await;
            if input_stream.peek().await == Some('.') {
                input_stream.next().await;
            }
        }
    })
}
