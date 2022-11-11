use super::instruction::*;
use super::location::*;
use super::message::*;
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
    /// Converts a sequence of expressions to instructions, generating a new sequence that leaves a single result on the stack
    ///
    pub fn sequence_to_instructions(sequence: impl IntoIterator<Item=TalkExpression>) -> Vec<TalkInstruction<TalkLiteral, TalkSymbol>> {
        let mut result = vec![];

        for expr in sequence {
            if result.len() != 0 {
                result.push(TalkInstruction::Discard);
            }

            result.extend(expr.to_instructions());
        }

        result
    }

    ///
    /// 'Flattens' this expression to a series of instructions. Evaluating an expression always leaves one value behind on the stack.
    ///
    pub fn to_instructions(self) -> Vec<TalkInstruction<TalkLiteral, TalkSymbol>> {
        use TalkExpression::*;

        lazy_static! {
            /// Temporary storage used to store the 'primary' in a cascaded expression
            static ref CASCADE_PRIMARY_RESULT: TalkSymbol = TalkSymbol::new_unnamed(); 
        }

        match self {
            Empty                               => vec![TalkInstruction::LoadNil],
            AtLocation(location, expr)          => vec![vec![TalkInstruction::Location(location)], expr.to_instructions()].into_iter().flatten().collect(),
            WithComment(_comment, expr)         => expr.to_instructions(),
            Literal(literal)                    => vec![TalkInstruction::Load(literal)],
            Identifier(identifier)              => vec![TalkInstruction::LoadFromSymbol(TalkSymbol::from(identifier))],
            Return(expr)                        => expr.to_instructions(),
            Block(variables, expressions)       => vec![TalkInstruction::LoadBlock(variables.into_iter().map(|var| TalkSymbol::from(&*var)).collect(), Arc::new(Self::sequence_to_instructions(expressions)))],

            Assignment(name, expr)              => // Create result, duplicate it, store the value, return value is duplicated
                vec![
                    expr.to_instructions(), 
                    vec![TalkInstruction::Duplicate, TalkInstruction::StoreAtSymbol(TalkSymbol::from(name))]
                ].into_iter().flatten().collect(),
                
            VariableDeclaration(variables)      => // Evaluates to 'nil', creates new local bindings
                vec![
                    vec![TalkInstruction::LoadNil], 
                    variables.into_iter().map(|var| TalkInstruction::PushLocalBinding(TalkSymbol::from(var))).collect()
                ].into_iter().flatten().collect(),

            SendMessage(expr, arguments)        => { // Evaluate the expression, the arguments, then sends a message
                let signature       = TalkMessageSignature::from_expression_arguments(&arguments);

                vec![
                    expr.to_instructions(),
                    arguments.iter().rev().flat_map(|arg| arg.value.clone()).flat_map(|expr| expr.to_instructions()).collect(),
                    vec![TalkInstruction::SendMessage(signature.id(), signature.len())]
                ].into_iter().flatten().collect()
            },

            CascadeFrom(expr, expressions)      =>  // Store the primary expression in CASCADE_PRIMARY_RESULT, evaluate the expressions, discard CASCADE_PRIMARY_RESULT
                vec![
                    expr.to_instructions(),
                    vec![TalkInstruction::PushLocalBinding(*CASCADE_PRIMARY_RESULT), TalkInstruction::StoreAtSymbol(*CASCADE_PRIMARY_RESULT)],
                    Self::sequence_to_instructions(expressions),
                    vec![TalkInstruction::LoadNil, TalkInstruction::StoreAtSymbol(*CASCADE_PRIMARY_RESULT), TalkInstruction::PopLocalBinding(*CASCADE_PRIMARY_RESULT)],
                ].into_iter().flatten().collect(),
            CascadePrimaryResult                => vec![TalkInstruction::LoadFromSymbol(*CASCADE_PRIMARY_RESULT)],
         }
    }
}
