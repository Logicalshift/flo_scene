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

    /// A symbol (`#'foo'` or `#foo`)
    Symbol(Arc<String>),

    /// An array (`#(1 2 3 4)`)
    Array(Vec<TalkLiteral>),

    /// A block of expressions
    Block(Vec<TalkExpression>),
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
    /// A literal
    Literal(TalkLiteral),

    /// A variable declaration (`| a b foo |`) 
    VariableDeclaration(Vec<Arc<String>>),

    /// Set a variable to the result of a program (`a := 42`)
    Assignment(String, Box<TalkExpression>),

    /// Send a message with arguments
    SendMessage(Box<TalkExpression>, Arc<String>, Vec<TalkArgument>),
}

///
/// A flotalk program consists of a series of expressions
///
pub struct TalkProgram(pub Vec<TalkExpression>);
