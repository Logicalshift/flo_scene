# Command interpreter

This is the command interpreter for `flo_scene`, a component platform for Rust. Commands here
can be used to interact with a scene in an active program.

The command is the `help` command, which can provide information about using the command interpreter
or documentation on flo_scene itself. You can specify a topic as a parameter to get more specific
help on something. Parameters are parsed as JSON, so they need to be enclosed in quotes, for example
`help "topics"` to see a complete list of available help topics.

As well as a topic, you can specify a command, so `help "echo"` will provide information on the 'echo'
command.

## Getting started

See these topics for some more information on getting started with the command interepreter:

| | |
| - | - |
| help "syntax" | Describes the syntax supported by the command interpreter |
| help "flo_scene_version" | Displays the version of flo_scene that is running |
| help "commands" | Lists all of the commands that are available in the interpreter |
| help "topics" | Produces an index of all of the other topics that are available in the help system |
