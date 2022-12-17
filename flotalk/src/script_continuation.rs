use super::continuation::*;
use super::expression::*;
use super::parser::*;
use super::simple_evaluator::*;
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

        // Run the insutrctions
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
