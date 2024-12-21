use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;

///
/// The stream command, which turns its argument into a JSON stream result
///
pub async fn command_stream(source: Vec<serde_json::Value>, _context: SceneContext) -> CommandResponse {
    CommandResponse::IoStream(Box::new(|_input_stream| stream::iter(source).boxed()))
}
