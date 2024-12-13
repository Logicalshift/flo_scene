use crate::commands::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;
use itertools::Itertools;
use once_cell::sync::{Lazy};

use std::borrow::{Cow};
use std::collections::{HashSet, HashMap};

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

///
/// Response from a request for the markdown for a help topic
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpMarkdown(pub Cow<'static, str>);

/// Stores the help data for topics
struct Topic { 
    hidden:         bool,
    description:    String,
    markdown:       Cow<'static, str>
}

/// Stores the help data for the commands
struct Command {
    description:    String,
    markdown:       Cow<'static, str>,
}

///
/// Reads the commands from the current context
///
/// Subprograms are only queried for their commands if they're not already in the known_subprograms list, which is also updated on return to
/// include the list of programs that were queried during this pass.
///
async fn read_new_commands(context: &SceneContext, known_subprograms: &mut HashSet<SubProgramId>) -> Vec<(String, Command)> {
    // Query the subprograms from the control program
    let scene_status = context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), ());
    let scene_status = if let Ok(scene_status) = scene_status {
        scene_status
    } else {
        return vec![];
    };

    // Figure out which subprograms have been added
    let active_subprograms  = scene_status.flat_map(|update| match update {
        SceneUpdate::Started(program_id, _) => stream::iter(std::iter::once(program_id)).boxed(),
        _                                   => stream::empty().boxed(),
    }).collect::<HashSet<SubProgramId>>().await;
    let added_subprograms = active_subprograms.iter()
        .filter(|new_program| !known_subprograms.contains(new_program))
        .copied()
        .collect::<HashSet<_>>();

    // Query each of the added subprograms to get the commands
    let mut new_commands = vec![];

    for added_program_id in added_subprograms {
        // Add to the list of known commands
        known_subprograms.insert(added_program_id);

        if let Ok(supported_commands) = context.spawn_query(ReadCommand::default(), RunCommand::<JsonParameter, CommandResponse>::new((), LIST_COMMANDS, ()), added_program_id) {
            let mut supported_commands = supported_commands;

            // Create Command objects for each command
            while let Some(cmd) = supported_commands.next().await {
                let cmd: ListCommandResponse = if let Ok(cmd) = cmd.try_into() { cmd } else { continue; };

                // Commands get an empty help topic
                for description in cmd.0 {
                    // Request more details about this command
                    let describe_result = context.spawn_query(ReadCommand::default(), RunCommand::<JsonParameter, CommandResponse>::new((), DESCRIBE_COMMAND, DescribeCommandRequest(description.name.clone())), added_program_id);

                    if let Ok(mut describe_result) = describe_result {
                        if let Some(describe_result) = describe_result.next().await {
                            if let Ok(describe_result) = describe_result.try_into() {
                                let describe_result: DescribeCommandResponse = describe_result;

                                // Retrieved description for this command
                                new_commands.push((description.name.clone(), Command { description: describe_result.summary, markdown: describe_result.help }));
                            }
                        } else {
                            // No description available for this command
                            new_commands.push((description.name.clone(), Command { description: "".into(), markdown: "".into() }));
                        }
                    }
                }
            }
        }
    }

    new_commands
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
                // The topics store the messages we return for different help requests, with the exception of a few custom ones that do things like list the available commands
                let mut topics      = HashMap::new();
                let mut commands    = HashMap::new();
                let mut subprograms = HashSet::new();

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
                            // Update the list of known commands
                            let new_commands = read_new_commands(&context, &mut subprograms).await;
                            for (name, details) in new_commands {
                                if !commands.contains_key(&name) {
                                    commands.insert(name, details);
                                }
                            }

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
                                command.markdown.clone()
                            } else if let Some(markdown) = topics.get(&topic) {
                                // Otherwise look up topics
                                markdown.markdown.clone()
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
                                target.send(QueryResponse::with_data(HelpMarkdown(markdown))).await.ok();
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

impl SceneMessage for HelpMarkdown { 
}

impl QueryRequest for HelpQueryTopic {
    type ResponseData = HelpMarkdown;

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
                    return CommandResponse::Markdown(markdown.0.into());
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
