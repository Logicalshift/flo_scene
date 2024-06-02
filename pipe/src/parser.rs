use futures::future::{LocalBoxFuture};

use std::collections::{VecDeque};

pub use super::tokenizer::*;
pub use super::parse_json::*;
pub use super::parse_command::*;

///
/// Error indicating that a token cannot be accepted by the parser because there is no token in the lookahead
///
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ParserLookaheadEmpty;

///
/// Error indicating there are not enough tokens in the parser stack to perform a 'reduce' operation
///
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ParserStackTooSmall;

///
/// Error indicating that `finish()` was called when the stack does not have exactly one treenode item in it
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserDidNotConverge<TToken, TTreeNode>(pub Vec<ParserStackEntry<TToken, TTreeNode>>);

///
/// An entry for the parser stack used by the ParserClass
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParserStackEntry<TToken, TTreeNode> {
    /// Represents a token that has been accepted from the tokenizer but has not been incorporated into a tree node yet
    Token(TToken),

    /// Represents a tree node (which is also the final result of the parser)
    Node(TTreeNode),
}

///
/// Simple parser type that can be used to convert an input stream of tokens into a tree (of a particular tree node type)
///
/// This can be used with the `Tokenizer` to parse the contents of a stream, and is easiest to use to create a left-recursive
/// recursive-descent style parser.
///
#[derive(Clone)]
pub struct Parser<TToken, TTreeNode> {
    /// The stack of nodes recognised by the parser. Should end up with a single 'Node' entry if the parsing is successful
    stack: Vec<ParserStackEntry<TToken, TTreeNode>>,

    /// Lookahead tokens from the tokenizer
    lookahead: VecDeque<TToken>,
}

impl<TToken, TTreeNode> Parser<TToken, TTreeNode> {
    ///
    /// Creates a new parser ready to accept tokens
    ///
    pub fn new() -> Self {
        Self {
            stack:      Vec::with_capacity(32),
            lookahead:  VecDeque::with_capacity(8),
        }
    }

    ///
    /// Attempts to look ahead by the specified number of tokens and returns what's there. Returns 'None' if the lookahead is beyond the end of file marker.
    ///
    pub async fn lookahead<'a, TTokenizer>(&'a mut self, distance: usize, tokenizer: &mut TTokenizer, read_token: impl 'a + Fn(&mut TTokenizer) -> LocalBoxFuture<'_, Option<TToken>>) -> Option<&'a TToken> {
        // Fill the lookahead until the token is available
        while self.lookahead.len() <= distance {
            if let Some(next_token) = read_token(tokenizer).await {
                // Add the next token
                self.lookahead.push_back(next_token);
            } else {
                // Reached the end of file
                break;
            }
        }

        self.lookahead.get(distance)
    }

    ///
    /// Ensures that lookahead is available up until 'distance' tokens ahead (so that `accept_expected_token` will work).
    ///
    pub async fn ensure_lookahead<'a, TTokenizer>(&'a mut self, distance: usize, tokenizer: &mut TTokenizer, read_token: impl 'a + Fn(&mut TTokenizer) -> LocalBoxFuture<'_, Option<TToken>>) -> &mut Self {
        self.lookahead(distance, tokenizer, read_token).await;

        self
    }

    ///
    /// Accepts the token that's currently the first lookahead, adding it to the stack as a Token. Call `lookahead(0)` to ensure that this token exists.
    ///
    /// Returns `Err(ParserLookaheadEmpty` if there is no lookahead, or `Ok(&mut Parser)` if valid, to allow chaining when performing a lot of stack actions
    ///
    #[inline]
    pub fn accept_token(&mut self) -> Result<&mut Self, ParserLookaheadEmpty> {
        if let Some(lookahead) = self.lookahead.pop_front() {
            self.stack.push(ParserStackEntry::Token(lookahead));

            Ok(self)
        } else {
            Err(ParserLookaheadEmpty)
        }
    }

    ///
    /// Discards the token that's currently the first lookahead
    ///
    /// If there's no first lookahead, then this is a no-op. Returns &mut Parser to allow chaining when a lot of operations are being performed
    ///
    #[inline]
    pub fn skip_token(&mut self) -> &mut Self {
        self.lookahead.pop_front();

        self
    }

    ///
    /// Drains the lookahead in this parser so the tokens can be returned to the tokenizer if necessary
    ///
    /// This can be useful for switching between different tokenizers, where the raw values that were tokenized need to be
    /// returned to the tokenizer's lookahead to be re-used.
    ///
    #[inline]
    pub fn return_lookahead(&mut self) -> impl '_ + Iterator<Item=TToken> {
        self.lookahead.drain(..)
    }

    ///
    /// Pops `len` elements from the stack, and converts them via the specified function into a tree node
    ///
    #[inline]
    pub fn reduce(&mut self, len: usize, reduce_fn: impl FnOnce(Vec<ParserStackEntry<TToken, TTreeNode>>) -> TTreeNode) -> Result<&mut Self, ParserStackTooSmall> {
        if self.stack.len() >= len {
            // Remove the specified number of entries from the stack and then convert them into a tree node via the reduction function
            let reduced_entries = self.stack.split_off(self.stack.len() - len);
            let new_node        = reduce_fn(reduced_entries);

            self.stack.push(ParserStackEntry::Node(new_node));

            Ok(self)
        } else {
            Err(ParserStackTooSmall)
        }
    }

    ///
    /// Finishes this parser, returning the node that the input was reduced to, or the stack if the parser did not converge to a single node
    ///
    pub fn finish(mut self) -> Result<TTreeNode, ParserDidNotConverge<TToken, TTreeNode>> {
        if self.stack.len() == 1 {
            if let Some(ParserStackEntry::Node(result)) = self.stack.pop() {
                Ok(result)
            } else {
                Err(ParserDidNotConverge(self.stack))
            }
        } else {
            Err(ParserDidNotConverge(self.stack))
        }
    }

    ///
    /// As for 'accept_token' except we first check that the lookahead matches a specific token. This will return an error if the lookahead has not been loaded
    /// yet: call `ensure_lookahead` (or just `lookahead`) to expand the lookahead to at least one token before using this.
    ///
    pub fn accept_expected_token(&mut self, matches: impl Fn(&TToken) -> bool) -> Result<&mut Self, ParserLookaheadEmpty> {
        if let Some(lookahead) = self.lookahead.get(0) {
            if matches(lookahead) {
                self.stack.push(ParserStackEntry::Token(self.lookahead.pop_front().unwrap()));

                Ok(self)
            } else {
                Err(ParserLookaheadEmpty)
            }
        } else {
            Err(ParserLookaheadEmpty)
        }
    }
}

impl<TToken, TTreeNode> ParserStackEntry<TToken, TTreeNode> {
    ///
    /// Reads the value that's in this stack entry if it's a node
    ///
    #[inline]
    pub fn node(&self) -> Option<&TTreeNode> {
        match self {
            ParserStackEntry::Node(node)    => Some(node),
            _                               => None
        }
    }

    ///
    /// Reads the value that's in this stack entry if it's a node
    ///
    #[inline]
    pub fn to_node(self) -> Option<TTreeNode> {
        match self {
            ParserStackEntry::Node(node)    => Some(node),
            _                               => None
        }
    }

    ///
    /// Reads the value that's in this stack entry if it's a token
    ///
    #[inline]
    pub fn token(&self) -> Option<&TToken> {
        match self {
            ParserStackEntry::Token(token)  => Some(token),
            _                               => None
        }
    }

    ///
    /// Reads the value that's in this stack entry if it's a token
    ///
    #[inline]
    pub fn to_token(self) -> Option<TToken> {
        match self {
            ParserStackEntry::Token(token)  => Some(token),
            _                               => None
        }
    }
}
