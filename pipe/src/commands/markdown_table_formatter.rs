//!
//! We format tables in two passes: first pass measures each column using the maximum width, which we use to figure out
//! the actual formatting width for each column. We try to use the full width of the column where possible, choosing
//! to resize the largest columns that overflow the width of the screen (which might not always produce ideal results)
//!

use super::markdown_formatter::*;

use comrak;

use std::cell::{RefCell};

///
/// Formats a table to a target formatter
///
pub fn table_format(target: &mut Formatter, table: &comrak::nodes::NodeTable, table_node_children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
    
}