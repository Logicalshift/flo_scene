use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;

static DEFAULT_MSG: &'static [u8] = include_bytes!("help-intro.md");

///
/// The 'help' command, which generates some help text
///
pub fn command_help(input: Option<String>, _context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Markdown(String::from_utf8(DEFAULT_MSG.into()).unwrap())
    }
}
