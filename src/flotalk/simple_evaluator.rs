use super::continuation::*;
use super::expression::*;
use super::instruction::*;
use super::symbol::*;
use super::value::*;
use super::value_store::*;

use std::sync::*;

///
/// Evaluates a FloTalk expression which does not have any binding specified, and where Literals have not been parsed into values
///
/// This is the simplest form of expression evaluator, which runs the slowest out of all the possible implementations (due to needing to parse values and look up
/// symbols every time)
///
pub fn talk_evaluate_simple(root_values: Arc<Mutex<TalkValueStore<TalkValue>>>, expression: Arc<Vec<TalkInstruction<TalkLiteral, TalkSymbol>>>) -> TalkContinuation {
    todo!()
}
