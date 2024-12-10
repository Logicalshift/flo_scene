use crate::commands::*;

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;
use itertools::Itertools;

use std::borrow::{Cow};
use std::collections::{HashMap};

static DEFAULT_MSG: &'static [u8] = include_bytes!("help-intro.md");

///
/// Message used to configure the responses to the 'help' command in a scene
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandHelp {
    /// Queries the markdown text for a help topic (returning a query response with the markdown for this topic)
    Query(StreamTarget, String),

    /// Sets the markdown to return for a particular topic
    AddTopic { topic: String, markdown: Cow<'static, str> },

    /// Adds a topic that can be retrieved but isn't listed in the index
    AddHiddenTopic { topic: String, markdown: Cow<'static, str> },

    /// Sets the markdown and description of a command
    AddCommand { command_name: String, description: String, markdown: Cow<'static, str>}
}

impl SceneMessage for CommandHelp {
    fn default_target() -> StreamTarget {
        StreamTarget::Program(SubProgramId::called("flo_scene_pipe::CommandHelp"))
    }

    fn initialise(scene: &Scene) {
        // Default behaviour for the CommandHelp subprogram
        scene.add_subprogram(SubProgramId::called(
            "flo_scene_pipe::CommandHelp"), 
            |input, context| async move {
                struct Topic { 
                    hidden:     bool,
                    markdown:   Cow<'static, str>
                }

                struct Command {
                    description:    String,
                    markdown:       Cow<'static, str>,
                }

                // The topics store the messages we return for different help requests, with the exception of a few custom ones that do things like list the available commands
                let mut topics      = HashMap::new();
                let mut commands    = HashMap::new();

                topics.insert("".to_string(), Topic { hidden: true, markdown: String::from_utf8_lossy(DEFAULT_MSG) });
                topics.insert("about".to_string(), Topic { hidden: false, markdown: format!("flo_scene {}", env!("CARGO_PKG_VERSION")).into() });

                // Process CommandHelp requests
                let mut input = input;
                while let Some(command_help) = input.next().await {
                    match command_help {
                        CommandHelp::AddTopic { topic, markdown } => {
                            topics.insert(topic, Topic { hidden: false, markdown: markdown } );
                        }

                        CommandHelp::AddHiddenTopic { topic, markdown } => {
                            topics.insert(topic, Topic { hidden: true, markdown: markdown } );
                        }

                        CommandHelp::AddCommand { command_name, description, markdown } => {
                            commands.insert(command_name, Command { description: description, markdown: markdown.into() });
                        }

                        CommandHelp::Query(target, topic) => {
                            let markdown = if let Some(command) = commands.get(&topic) {
                                // If there's a command, this takes priority over the topic, if there's one that matches
                                command.markdown.clone()
                            } else if let Some(markdown) = topics.get(&topic) {
                                // Otherwise look up topics
                                markdown.markdown.clone()
                            } else {
                                // If a request is made for a topic with no data, we produce a list of all the non-hidden topics
                                format!("# Help topic '{}' not known\n\nThis topic is not in the list of topics known about by this help system.\n\nAvailable topics are:\n\n{}",
                                    topic,
                                    topics.iter()
                                        .filter(|(_, topic_description)| !topic_description.hidden)
                                        .map(|topic| topic.0)
                                        .sorted()
                                        .map(|topic| format!("| {} |\n", topic))
                                        .collect::<String>()
                                    ).into()
                            };

                            if let Ok(mut target) = context.send(target) {
                                target.send(QueryResponse::with_data(markdown.clone())).await.ok();
                            }
                        }
                    }
                }
            },
            20);
    }
}

///
/// The 'help' command, which generates some help text
///
pub fn command_help(input: Option<String>, _context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Markdown(String::from_utf8(DEFAULT_MSG.into()).unwrap())
    }
}
