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
pub fn table_format<'a>(target: &mut Formatter, table_node: &'a comrak::arena_tree::Node<'a, RefCell<comrak::nodes::Ast>>) {
    let available_width = target.available_width();

    // Add some padding after the last paragraph
    target.commit_current_word();
    target.newline();
    target.newline();

    //
    // PASS 1: calculate the widths of the columns in the table
    //
    let mut column_widths = vec![];

    for row in table_node.children() {
        use comrak::nodes::{NodeValue};

        if let NodeValue::TableRow(_) = &row.data.borrow().value {
            for (column_idx, column) in row.children().enumerate() {
                // Ensure that there are enough columns to accomodate the current width
                while column_widths.len() <= column_idx { column_widths.push(0); }

                // Measure the required width of this cell
                if let NodeValue::TableCell = &column.data.borrow().value {
                    // Format this cell using the maximum available width
                    let mut cell_formatter = Formatter::new(available_width, 0);
                    cell_formatter.format("", "", column.children());
                    cell_formatter.commit_current_word();

                    column_widths[column_idx] = column_widths[column_idx].max(cell_formatter.max_xpos());
                }
            }
        }
    }

    // 'No cells' is a weird special case, we just don't do anything for that
    if column_widths.len() == 0 {
        return;
    }

    // Figure out the actual column widths to use
    // 3 chars between each column, + 2 chars on the end points
    let total_width = column_widths.iter().sum::<usize>() + ((column_widths.len()-1) * 3) + 4;
    let mut last_row_was_header = false;

    if total_width > available_width {
        // Total size available across all columns (allowing for 2 chars at the start & end + 3 between columns)
        let available_for_columns   = available_width - 4 - ((column_widths.len() - 1) * 3);

        // Adjusted widths are made to fit the whole table
        let mut remaining_width     = available_for_columns;
        let mut adjusted_widths     = vec![];

        for (column_idx, column_width) in column_widths.iter().enumerate() {
            // We try to resize all columns to use an even amount of space, but allow them to be smaller than this
            let num_remaining_columns   = (column_widths.len()-1) - column_idx;
            let max_column_width        = remaining_width / (num_remaining_columns + 1);

            // Use the existing width if it's already smaller than what's needed
            let new_width = if *column_width > max_column_width { max_column_width } else { *column_width };

            // Add the adjusted width
            adjusted_widths.push(new_width);
            remaining_width -= new_width;
        }

        column_widths = adjusted_widths;
    }

    //
    // PASS 2: format the table
    //
    for (row_idx, row) in table_node.children().enumerate() {
        use comrak::nodes::{NodeValue};

        if let NodeValue::TableRow(is_header_row) = &row.data.borrow().value {
            let mut formatted_cells = vec![];

            // Draw the lines for the header of this row
            let mut header_lines = String::new();
            if row_idx == 0 { header_lines.push('\u{256d}'); } else if last_row_was_header { header_lines.push('\u{255e}'); } else { header_lines.push('\u{251c}'); }

            for (idx, width) in column_widths.iter().copied().enumerate() {
                let is_last_column = (idx+1) == column_widths.len();

                if last_row_was_header { header_lines.extend((0..(width+2)).map(|_| '\u{2550}')); } else { header_lines.extend((0..(width+2)).map(|_| '\u{2500}')); };
                if !is_last_column {
                    if row_idx == 0 { header_lines.push('\u{252c}'); } else if last_row_was_header { header_lines.push('\u{256a}'); } else { header_lines.push('\u{253c}'); }
                }
            }

            if row_idx == 0 { header_lines.extend("\u{256e}".chars()); } else if last_row_was_header { header_lines.push('\u{2561}'); } else { header_lines.extend("\u{2524}".chars()); }

            target.append_raw(&header_lines, available_width);
            target.newline();

            // The header row uses a double line for the formatting
            last_row_was_header = *is_header_row;

            // Format the text for the cells (which may be multi-line)
            for (column_idx, column) in row.children().enumerate() {
                // Ensure that there are enough columns to accomodate the current width
                while column_widths.len() <= column_idx { column_widths.push(0); }

                // Measure the required width of this cell
                if let NodeValue::TableCell = &column.data.borrow().value {
                    // Format this cell using the column width we've decided on. Use padding so it fits in the table
                    let mut cell_formatter = Formatter::new(column_widths[column_idx], 0);
                    cell_formatter.set_pad_to_width(true);
                    cell_formatter.format("", "", column.children());
                    cell_formatter.commit_current_word();

                    // Terminating newline is required
                    cell_formatter.newline();

                    // Add to the formatted cells for the current row
                    formatted_cells.push(cell_formatter.to_string(false));
                }
            }

            // Render the lines in each of the formatted cells (relying on the padding + newlines, last newline is ignored)
            let mut cell_readers = formatted_cells.iter().map(|cell| Some(cell.chars())).collect::<Vec<_>>();

            loop {
                // Each reader is just before the first character of the cell (or is None). We're done with this table row once all the cells are rendered
                let mut line = String::new();

                for (column_idx, maybe_reader) in cell_readers.iter_mut().enumerate() {
                    line.push('\u{2502}');
                    line.push(' ');

                    if let Some(reader) = maybe_reader {
                        loop {
                            // Add to the line until we hit a newline or the end of the reader
                            match reader.next() {
                                Some('\n')  => { break; }
                                Some(chr)   => { line.push(chr); },
                                None        => { *maybe_reader = None; break; }
                            }
                        }
                    } else {
                        // Cell is finished, pad out with spaces
                        line.extend((0..column_widths[column_idx]).map(|_| ' '));
                    }

                    line.push(' ');
                }
                line.push('\u{2502}');

                // Stop once all the readers have finished
                if cell_readers.iter().all(|reader| reader.is_none()) {
                    break;
                }

                // Append the line
                target.append_raw(&line, available_width);
                target.newline();
            }
        }
    }

    // Close off the table
    let mut header_lines = String::new();
    header_lines.extend("\u{2514}".chars());

    for (idx, width) in column_widths.iter().copied().enumerate() {
        let is_last_column = (idx+1) == column_widths.len();

        header_lines.extend((0..(width+2)).map(|_| '\u{2500}'));
        if !is_last_column {
            header_lines.push('\u{2534}');
        }
    }

    header_lines.extend("\u{2518}".chars());

    target.append_raw(&header_lines, available_width);
}
