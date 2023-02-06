use super::continuation::*;
use super::expression::*;
use super::instruction::*;
use super::parse_error::*;
use super::parser::*;
use super::reference::*;
use super::simple_evaluator::*;
use super::symbol::*;
use super::symbol_table::*;
use super::value::*;

use futures::prelude::*;

use std::sync::*;

///
/// A FloTalk script in string format
///
#[derive(Clone, PartialEq)]
pub struct TalkScript(pub String);

impl From<&str> for TalkScript {
    fn from(script: &str) -> TalkScript {
        TalkScript(script.to_string())
    }
}

impl From<String> for TalkScript {
    fn from(script: String) -> TalkScript {
        TalkScript(script)
    }
}

impl From<Arc<String>> for TalkScript {
    fn from(script: Arc<String>) -> TalkScript {
        TalkScript(Arc::try_unwrap(script).unwrap_or_else(|script| (*script).clone()))
    }
}

impl From<&Arc<String>> for TalkScript {
    fn from(script: &Arc<String>) -> TalkScript {
        TalkScript((**script).clone())
    }
}

impl TalkScript {
    ///
    /// Converts this script to a list of expressions
    ///
    pub async fn to_expressions(&self) -> Result<Vec<TalkExpression>, TalkParseError> {
        let source_stream   = stream::iter(self.0.chars());
        let mut expressions = parse_flotalk_expression(source_stream);
        let mut program     = vec![];

        while let Some(next_expression) = expressions.next().await {
            match next_expression {
                Ok(next_expression) => { program.push(next_expression.value); },
                Err(parser_error)   => { return Err(parser_error.value.into()).into(); }
            }
        }

        Ok(program)
    }

    ///
    /// Converts this script to some instructions
    ///
    pub async fn to_instructions(&self) -> Result<Vec<TalkInstruction<TalkLiteral, TalkSymbol>>, TalkParseError> {
        let program         = self.to_expressions().await;
        let program         = match program { Ok(x) => x, Err(e) => { return Err(e); } };
        let instructions    = TalkExpression::sequence_to_instructions(program);

        Ok(instructions)
    }
}

///
/// Creates a continuation that runs the specified script with a set of frames (assumed to be retained)
///
pub fn continuation_from_script_with_symbol_table<'a>(script: impl Into<TalkScript>, symbol_table: Arc<Mutex<TalkSymbolTable>>, frames: Vec<TalkCellBlock>) -> TalkContinuation<'a> {
    let TalkScript(source) = script.into();

    TalkContinuation::future_soon(async move { 
        // Parse all the parts of the script
        let source_stream   = stream::iter(source.chars());
        let mut expressions = parse_flotalk_expression(source_stream);
        let mut program     = vec![];

        while let Some(next_expression) = expressions.next().await {
            match next_expression {
                Ok(next_expression) => { program.push(next_expression.value); },
                Err(parser_error)   => { return TalkValue::Error(parser_error.value.into()).into(); }
            }
        }

        // Convert to instructions
        let instructions = TalkExpression::sequence_to_instructions(program);
        let instructions = Arc::new(instructions);

        // Run the instructions
        TalkContinuation::soon(move |talk_context| {
            // Any new symbols are local to the evaluation
            let eval_symbol_table = Arc::new(Mutex::new(TalkSymbolTable::with_parent_frame(symbol_table)));
            let eval_symbol_block = talk_context.allocate_cell_block(1);

            let mut frames = frames;
            frames.insert(0, eval_symbol_block);

            let release_frames = frames.clone();

            // Evaluate the expression, then release the cell blocks
            talk_evaluate_simple(eval_symbol_table, frames, instructions)
                .and_then(move |result| {
                    TalkContinuation::soon(move |talk_context| {
                        release_frames.into_iter().for_each(|cell_block| { talk_context.release_cell_block(cell_block); });

                        result.into()
                    })
                })
        })
    })
}

///
/// Creates a continuation that can run the specified script
///
pub fn continuation_from_script<'a>(script: impl Into<TalkScript>) -> TalkContinuation<'a> {
    let TalkScript(source) = script.into();

    TalkContinuation::future_soon(async move { 
        // Parse all the parts of the script
        let source_stream   = stream::iter(source.chars());
        let mut expressions = parse_flotalk_expression(source_stream);
        let mut program     = vec![];

        while let Some(next_expression) = expressions.next().await {
            match next_expression {
                Ok(next_expression) => { program.push(next_expression.value); },
                Err(parser_error)   => { return TalkValue::Error(parser_error.value.into()).into(); }
            }
        }

        // Convert to instructions
        let instructions = TalkExpression::sequence_to_instructions(program);
        let instructions = Arc::new(instructions);

        // Run the instructions
        TalkContinuation::soon(move |talk_context| {
            // Run with the root symbol table
            let root_symbol_table = talk_context.root_symbol_table();
            let root_symbol_block = talk_context.root_symbol_table_cell_block().leak();

            // Any new symbols are local to the evaluation
            let eval_symbol_table = Arc::new(Mutex::new(TalkSymbolTable::with_parent_frame(root_symbol_table)));
            let eval_symbol_block = talk_context.allocate_cell_block(1);

            // Evaluate the expression, then release the cell blocks
            talk_evaluate_simple(eval_symbol_table, vec![eval_symbol_block.clone(), root_symbol_block.clone()], instructions)
                .and_then(move |result| {
                    TalkContinuation::soon(move |talk_context| {
                        talk_context.release_cell_block(root_symbol_block);
                        talk_context.release_cell_block(eval_symbol_block);

                        result.into()
                    })
                })
        })
    })
}

impl<'a> From<TalkScript> for TalkContinuation<'a> {
    fn from(script: TalkScript) -> TalkContinuation<'a> {
        continuation_from_script(script)
    }
}
