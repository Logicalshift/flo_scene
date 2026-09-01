use crate::commands::*;

use flo_scene::*;
use futures::prelude::*;
use futures::channel::oneshot;
use futures::channel::mpsc;

use serde::*;
use serde_json;
use comrak::*;

use std::collections::*;

///
/// Arguments to the 'tabulate' command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct TabulateArguments {
}

///
/// Generates a markdown table from a set of JSON values
///
fn tabulate<'a>(_arguments: &TabulateArguments, values: impl Iterator<Item=&'a serde_json::Value>) -> CommandResponse {
    use std::iter;

    let mut ordered_columns = vec![];
    let mut columns         = HashMap::new();
    let mut rows            = vec![];

    let mut values          = values;

    // We support two formats: list of objects (table headers are the keys) or a list of lists (table headers are the string values in the first row)
    let Some(first_value) = values.next() else { return CommandResponse::Markdown("| Empty |\n".into()) };

    match first_value {
        serde_json::Value::Array(values) => {
            // First value is the header values
            ordered_columns.extend(values.iter().map(|val| val.as_str().unwrap_or("").to_string()));
            columns.extend(ordered_columns.iter().cloned().enumerate().map(|(idx, val)| (val, idx)));

            // Other values are the rows
            for row in values {
                match row {
                    serde_json::Value::Array(values) => {
                        let next_row = values.iter().map(|column| column.to_string()).collect::<Vec<_>>();
                        rows.push(next_row);
                    },

                    _ => { }
                }
            }
        }

        serde_json::Value::Object(_) => {
            for row in iter::once(first_value).chain(values) {
                match row {
                    serde_json::Value::Object(map) => {
                        let mut row_values = vec![];

                        for (key, value) in map {
                            // Get the index for this column (or add an extra column)
                            let column_idx = if let Some(idx) = columns.get(key) {
                                *idx
                            } else {
                                let idx = ordered_columns.len();
                                columns.insert(key.clone(), idx);
                                ordered_columns.push(key.clone());
                                idx
                            };

                            row_values.push((column_idx, value.to_string()));
                        }

                        // Re-order to generate the row
                        row_values.sort_by(|(idx1, _), (idx2, _)| idx1.cmp(idx2));

                        // Actually generate the row
                        let mut next_row = vec![];

                        for (idx, val) in row_values {
                            // Pad out to the index
                            while next_row.len() < idx {
                                next_row.push(String::new());
                            }

                            // Add the value
                            next_row.push(val);
                        }

                        rows.push(next_row);
                    }

                    _ => { }
                }
            }
        },

        _ => { 
            // Flat list if the first value isn't an object or a value
            let mut markdown = format!("| Value |\n| - |\n| {} |", escape_commonmark_inline(&first_value.to_string()));

            for value in values {
                markdown.push_str(&format!("\n| {} |", escape_commonmark_inline(&value.to_string())));
            }

            return CommandResponse::Markdown(markdown.into());
        }
    }

    // Format the rows as markdown
    let headers = ordered_columns
        .iter()
        .map(|column_name| format!("| {} ", escape_commonmark_inline(column_name)))
        .collect::<Vec<_>>();

    let dividers = headers
        .iter()
        .map(|header| format!("| {} ", iter::repeat('-').take(header.len()-3).collect::<String>()))
        .collect::<Vec<_>>();

    let mut markdown = headers.join("");
    markdown.push_str(&format!("|\n{}|\n", dividers.join("")));

    for mut row in rows.into_iter() {
        // Pad out the row
        while row.len() < ordered_columns.len() {
            row.push(String::new());
        }

        let row = row
            .into_iter()
            .map(|column_value| format!("| {} ", escape_commonmark_inline(&column_value)))
            .collect::<String>();
        markdown.push_str(&row);
        markdown.push_str("|\n");
    }

    CommandResponse::Markdown(markdown.into())
}

///
/// Function that implements the 'tabulate' command, which reads from an input pipe and generates a table from it
/// in markdown format.
///
pub fn command_tabulate(arguments: TabulateArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        // Get the command responses
        let responses       = context.send(());
        let mut responses   = if let Ok(responses) = responses { responses } else { return CommandResponse::Error("Can't create responses".into()); };

        // Open an IO stream to read the responses
        let (_send_responses, recv_responses)   = mpsc::channel(16);
        let (send_input, recv_input)            = oneshot::channel();

        if responses.send(CommandResponse::IoStream(Box::new(move |input_stream| {
                send_input.send(input_stream).ok();
                recv_responses.boxed()
            }))).await.is_err() {
            return CommandResponse::Error("Could not create ouput stream".into());
        }

        // Receive the IO stream
        let Ok(mut input)       = recv_input.await else { return CommandResponse::Error("No input".into()) };
        let mut table_values    = vec![];

        while let Some(input_val) = input.next().await {
            // Arrays are tabulated immediately, JSON values are gathered to tabulate at the end
            match input_val {
                serde_json::Value::Array(values) => {
                    let table = tabulate(&arguments, values.iter());
                    responses.send(table).await.ok();
                },

                serde_json::Value::Object(map) => {
                    table_values.push(serde_json::Value::Object(map));
                },

                serde_json::Value::Null         |
                serde_json::Value::Bool(_)      |
                serde_json::Value::Number(_)    |
                serde_json::Value::String(_)    => { }
            }
        }

        // If there were JSON objects in the stream, then tabulate them last
        if !table_values.is_empty() {
            tabulate(&arguments, table_values.iter())
        } else {
            CommandResponse::Message("".into())
        }
    }
}
