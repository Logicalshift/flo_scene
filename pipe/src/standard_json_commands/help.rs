use crate::commands::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;
use itertools::Itertools;
use once_cell::sync::{Lazy};

use std::borrow::{Cow};
use std::collections::{HashMap};

/// Filter that maps the 'HelpQueryTopic' message to a CommandHelp
static HELP_QUERY_FILTER: Lazy<FilterHandle> = Lazy::new(|| 
    FilterHandle::for_filter(|stream: InputStream<HelpQueryTopic>| 
        stream.map(|HelpQueryTopic(target, topic): HelpQueryTopic| CommandHelp::Query(target, topic))));

static DEFAULT_MSG: &'static [u8] = include_bytes!("help-intro.md");

///
/// Message used to configure the responses to the 'help' command in a scene
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandHelp {
    /// Queries the markdown text for a help topic (returning a query response with the markdown for this topic)
    Query(StreamTarget, String),

    /// Sets the markdown to return for a particular topic
    AddTopic { topic: String, description: String, markdown: Cow<'static, str> },

    /// Adds a topic that can be retrieved but isn't listed in the index
    AddHiddenTopic { topic: String, description: String, markdown: Cow<'static, str> },

    /// Sets the markdown and description of a command
    AddCommand { command_name: String, description: String, markdown: Cow<'static, str>}
}

///
/// Request sent to query a help topic
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpQueryTopic(pub StreamTarget, pub String);

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
                    hidden:         bool,
                    description:    String,
                    markdown:       Cow<'static, str>
                }

                struct Command {
                    description:    String,
                    markdown:       Cow<'static, str>,
                }

                // The topics store the messages we return for different help requests, with the exception of a few custom ones that do things like list the available commands
                let mut topics      = HashMap::new();
                let mut commands    = HashMap::new();

                topics.insert("".to_string(), Topic { hidden: true, description: "Default help topic".into(), markdown: String::from_utf8_lossy(DEFAULT_MSG) });
                topics.insert("commands".to_string(), Topic { hidden: false, description: "Describe the available commands".into(), markdown: "".into() });
                topics.insert("flo_scene_version".to_string(), Topic { hidden: false, description: "Describe the version number of flo_scene that this was built on".into(), markdown: format!("flo_scene {}", env!("CARGO_PKG_VERSION")).into() });

                // Process CommandHelp requests
                let mut input = input;
                while let Some(command_help) = input.next().await {
                    match command_help {
                        CommandHelp::AddTopic { topic, description, markdown } => {
                            topics.insert(topic, Topic { hidden: false, description: description, markdown: markdown } );
                        }

                        CommandHelp::AddHiddenTopic { topic, description, markdown } => {
                            topics.insert(topic, Topic { hidden: true, description: description, markdown: markdown } );
                        }

                        CommandHelp::AddCommand { command_name, description, markdown } => {
                            commands.insert(command_name, Command { description: description, markdown: markdown.into() });
                        }

                        CommandHelp::Query(target, topic) => {
                            // The 'commands' topic is special in that it will list the commands with help attached to them
                            let markdown = if topic == "commands" {
                                format!("# Available commands:\n\n|   |   |\n| -- | -- |\n{}\n",
                                    commands.iter()
                                        .sorted_by_key(|(name, _)| *name)
                                        .map(|(name, description)| format!("| {} | {} |\n", name, description.description))
                                        .collect::<String>()
                                    ).into()
                            } else if let Some(command) = commands.get(&topic) {
                                // If there's a command, this takes priority over the topic, if there's one that matches
                                command.markdown.to_string()
                            } else if let Some(markdown) = topics.get(&topic) {
                                // Otherwise look up topics
                                markdown.markdown.to_string()
                            } else {
                                // If a request is made for a topic with no data, we produce a list of all the non-hidden topics
                                format!("# Help topic '{}' not known\n\nThis topic is not in the list of topics known about by this help system.\n\nAvailable topics are:\n\n| | |\n| -- | -- |\n{}",
                                    topic,
                                    topics.iter()
                                        .filter(|(_, topic_description)| !topic_description.hidden)
                                        .sorted_by_key(|(topic, _)| *topic)
                                        .map(|(topic, topic_description)| format!("| {} | {} |\n", topic, topic_description.description))
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

impl SceneMessage for HelpQueryTopic {
    fn default_target() -> StreamTarget {
        StreamTarget::Program(SubProgramId::called("flo_scene_pipe::CommandHelp"))
    }

    fn initialise(scene: &Scene) {
        // Convert help queries to CommandHelp requests
        scene.connect_programs(StreamSource::Filtered(*HELP_QUERY_FILTER), (), StreamId::with_message_type::<HelpQueryTopic>()).unwrap();
    }
}

impl QueryRequest for HelpQueryTopic {
    type ResponseData = String;

    fn with_new_target(self, new_target: StreamTarget) -> Self {
        HelpQueryTopic(new_target, self.1)
    }
}

impl HelpQueryTopic {
    ///
    /// Creates a new query for the specified help topic
    ///
    pub fn with_topic(topic: impl Into<String>) -> Self {
        Self(StreamTarget::Any, topic.into())
    }
}

///
/// The 'help' command, which generates some help text
///
pub fn command_help(input: Option<String>, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        let input = input.unwrap_or_else(|| "".into());

        match context.spawn_query(ReadCommand::default(), HelpQueryTopic::with_topic(input.clone()), ()) {
            Ok(markdown) => {
                // Send the resulting markdown to the target
                let mut markdown = markdown;
                while let Some(markdown) = markdown.next().await {
                    return CommandResponse::Markdown(markdown.into());
                }
            }

            Err(err) => {
                // The help program is not running
                return CommandResponse::Error(format!("Help is not available: {:?}", err));
            }
        }

        CommandResponse::Error(format!("Topic '{}' returned no help", input))
    }
}
