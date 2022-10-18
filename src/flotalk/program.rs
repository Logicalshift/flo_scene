use super::location::*;

use std::sync::*;

///
/// A literal from a flotalk program
///
#[derive(Clone, PartialEq, Debug)]
pub enum TalkLiteral {
    /// A number (`42`, `-42`, `123.45` etc)
    Number(Arc<String>),

    /// A character (`$A`)
    Character(char),

    /// A string (`'String'`)
    String(Arc<String>),

    /// A symbol (`#'foo'`)
    Symbol(Arc<String>),

    /// A selector (`#foo` or `#foo:`)
    Selector(Arc<String>),

    /// An array (`#(1 2 3 4)`)
    Array(Vec<TalkLiteral>),
}

///
/// An argument for a flotalk message 
///
#[derive(Clone, PartialEq, Debug)]
pub struct TalkArgument {
    /// Name of this argument
    pub name: Arc<String>,

    /// Expression that evaluates to the value of this argument
    pub value: TalkExpression,
}

///
/// Represents the AST of a flotalk expression
///
#[derive(Clone, PartialEq, Debug)]
pub enum TalkExpression {
    /// The empty expression `.`
    Empty,

    /// An expression that was parsed at a specific location (same as the boxed expression but the location can be used to highlight where any errors occurred)
    AtLocation(TalkLocation, Box<TalkExpression>),

    /// An expression that is preceded by a comment (`"The number 5" 5`), useful for documentation purposes
    WithComment(Arc<String>, Box<TalkExpression>),

    /// A literal
    Literal(TalkLiteral),

    /// A block of expressions
    Block(Vec<TalkExpression>),

    /// An identifier
    Identifier(Arc<String>),

    /// A variable declaration (`| a b foo | <expr>`) 
    VariableDeclaration(Vec<Arc<String>>),

    /// Set a variable to the result of a program (`a := 42`)
    Assignment(String, Box<TalkExpression>),

    /// A return expresson (expression starting with `^`)
    Return(Box<TalkExpression>),

    /// Send one or more messages with arguments
    SendMessages(Box<TalkExpression>, Vec<(Arc<String>, Vec<TalkArgument>)>),
}

///
/// A flotalk program consists of a series of expressions
///
pub struct TalkProgram(pub Vec<TalkExpression>);
