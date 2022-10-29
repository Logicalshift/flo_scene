use super::location::*;
use super::instruction::*;
use super::symbol::*;

use std::sync::*;

///
/// An identifier is something like `foo` but a keyword is an identifier followed by a ':', as in `foo:`.
///
#[derive(Clone, PartialEq, Debug)]
pub enum TalkIdentifierOrKeyword {
    /// Matched an identifier
    Identifier(Arc<String>),

    /// Matched a keyword
    Keyword(Arc<String>),
}

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
    pub value: Option<TalkExpression>,
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

    /// A code block (list of arguments and expressions)
    Block(Vec<Arc<String>>, Vec<TalkExpression>),

    /// An identifier
    Identifier(Arc<String>),

    /// A variable declaration (`| a b foo | <expr>`) 
    VariableDeclaration(Vec<Arc<String>>),

    /// Set a variable to the result of a program (`a := 42`)
    Assignment(Arc<String>, Box<TalkExpression>),

    /// A return expresson (expression starting with `^`)
    Return(Box<TalkExpression>),

    /// Send one or more messages with arguments
    SendMessage(Box<TalkExpression>, Vec<TalkArgument>),

    /// Cascade the result of a primary expression to a set of other expressions
    CascadeFrom(Box<TalkExpression>, Vec<TalkExpression>),

    /// The result of the primary from the `CascadeFrom` expression
    CascadePrimaryResult,
}

///
/// Argument to a method
///
#[derive(Clone, PartialEq, Debug)]
pub enum TalkMethodArgument {
    Unary(Arc<String>),
    Binary(Arc<String>, Arc<String>),
    Keyword(Vec<(Arc<String>, Arc<String>)>),
}

///
/// A method definition
///
#[derive(Clone, PartialEq, Debug)]
pub struct TalkMethodDefinition {
    /// Where in the input stream this method was encountered
    pub location: Option<TalkLocation>,

    /// If the method definition is preceded by a doc comment, this is it
    pub initial_comment: Option<Arc<String>>,

    /// The argument(s) for this method
    pub argument: TalkMethodArgument,

    /// The expressions that make up the method
    pub expressions: Vec<TalkExpression>,
}

impl TalkExpression {
    ///
    /// Strips out any comments or location information from the expression
    ///
    pub fn strip(self) -> TalkExpression {
        use TalkExpression::*;

        match self {
            Empty                               => self,
            AtLocation(_location, expr)         => expr.strip(),
            WithComment(_comment, expr)         => expr.strip(),
            Literal(ref _literal)               => self,
            Block(variables, expressions)       => Block(variables, expressions.into_iter().map(|expr| expr.strip()).collect()),
            Identifier(ref _identifier)         => self,
            VariableDeclaration(ref _variables) => self,
            Assignment(name, expr)              => Assignment(name, Box::new(expr.strip())),
            Return(expr)                        => Return(Box::new(expr.strip())),
            SendMessage(expr, arguments)        => SendMessage(Box::new(expr.strip()), arguments.into_iter().map(|arg| TalkArgument { name: arg.name, value: arg.value.map(|value| value.strip()) }).collect()),
            CascadeFrom(expr, expressions)      => CascadeFrom(Box::new(expr.strip()), expressions.into_iter().map(|expr| expr.strip()).collect()),
            CascadePrimaryResult                => self,
        }
    }

    ///
    /// Flattens an expression sequence, generating a new sequence that returns a result
    ///
    pub fn flatten_sequence(sequence: impl IntoIterator<Item=TalkExpression>) -> Vec<TalkFlatExpression<TalkLiteral, TalkSymbol>> {
        let mut result = vec![];

        for expr in sequence {
            if result.len() != 0 {
                result.push(TalkFlatExpression::Discard);
            }

            result.extend(expr.flatten());
        }

        result
    }

    ///
    /// Creates a 'flat' expression that can be evaluated with a stack
    ///
    pub fn flatten(self) -> Vec<TalkFlatExpression<TalkLiteral, TalkSymbol>> {
        use TalkExpression::*;

        lazy_static! {
            /// Temporary storage used to store the 'primary' in a cascaded expression
            static ref CASCADE_PRIMARY_RESULT: TalkSymbol = TalkSymbol::new_unnamed(); 
        }

        match self {
            Empty                               => vec![TalkFlatExpression::LoadNil],
            AtLocation(location, expr)          => vec![vec![TalkFlatExpression::Location(location)], expr.flatten()].into_iter().flatten().collect(),
            WithComment(_comment, expr)         => expr.flatten(),
            Literal(literal)                    => vec![TalkFlatExpression::Load(literal)],
            Identifier(identifier)              => vec![TalkFlatExpression::LoadFromSymbol(TalkSymbol::from(identifier))],
            Return(expr)                        => expr.flatten(),
            Block(variables, expressions)       => vec![TalkFlatExpression::LoadBlock(variables.into_iter().map(|var| TalkSymbol::from(&*var)).collect(), Self::flatten_sequence(expressions))],

            Assignment(name, expr)              => // Create result, duplicate it, store the value, return value is duplicated
                vec![
                    expr.flatten(), 
                    vec![TalkFlatExpression::Duplicate, TalkFlatExpression::StoreAtSymbol(TalkSymbol::from(name))]
                ].into_iter().flatten().collect(),
                
            VariableDeclaration(variables)      => // Evaluates to 'nil', creates new local bindings
                vec![
                    vec![TalkFlatExpression::LoadNil], 
                    variables.into_iter().map(|var| TalkFlatExpression::PushLocalBinding(TalkSymbol::from(var))).collect()
                ].into_iter().flatten().collect(),

            SendMessage(expr, arguments)        => // Evaluate the expression, the arguments, then sends a message
                vec![
                    expr.flatten(),
                    arguments.iter().flat_map(|arg| arg.value.clone()).flat_map(|expr| expr.flatten()).collect(),
                    vec![TalkFlatExpression::SendMessage(arguments.iter().map(|arg| TalkSymbol::from(&arg.name)).collect())]
                ].into_iter().flatten().collect(),

            CascadeFrom(expr, expressions)      =>  // Store the primary expression in CASCADE_PRIMARY_RESULT, evaluate the expressions, discard CASCADE_PRIMARY_RESULT
                vec![
                    expr.flatten(),
                    vec![TalkFlatExpression::PushLocalBinding(*CASCADE_PRIMARY_RESULT), TalkFlatExpression::StoreAtSymbol(*CASCADE_PRIMARY_RESULT)],
                    Self::flatten_sequence(expressions),
                    vec![TalkFlatExpression::LoadNil, TalkFlatExpression::StoreAtSymbol(*CASCADE_PRIMARY_RESULT), TalkFlatExpression::PopLocalBinding(*CASCADE_PRIMARY_RESULT)],
                ].into_iter().flatten().collect(),
            CascadePrimaryResult                => vec![TalkFlatExpression::LoadFromSymbol(*CASCADE_PRIMARY_RESULT)],
         }
    }
}
