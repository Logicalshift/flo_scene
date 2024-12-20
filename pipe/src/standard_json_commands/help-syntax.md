# Basics

A command can either be a single word (like `help`), or a single word followed by a single JSON-format 
parameter: `echo "Test"` or `echo { "Key": "Value" }`.

A JSON string by itself will evaluate as if it were a command returning that value. This is not very
useful by itself but can be combined with other features.

Commands are ended by a newline character, although JSON data can be stretched across several lines.

# Variables

Variables are names beginning with a `:`, for example `:my_variable`. These can be assigned the JSON
value returned by any command, for example `:my_variable = "Test"`, or `:my_variable = list_commands`

Variables create new commands that return the value of the variable.

```
> :foo = "Test"
> :foo
"Test"
```

Command substitution can be used to insert the value of a variable into a parameter:

```
> :foo = "Test"
> echo { "Key": <:foo> }
   {
     "Key": "Test"
   }
```

# Substitution

Commands that return JSON values can have those values substituted into parameters using the '<>' syntax.
For example, `echo <list_commands>` will write the JSON result of the `list_commands` command to the output.
This can also be used as part of the JSON syntax:

```
> echo { "Key": <list_commands> }
   {
     "Key": [ ... ]
   }
```

# Indexing

The `[]` operator can be appended to a JSON value in order to use a specific value. This is useful with
variables or substituted commands:

```
> echo <list_commands>[1]
   {
     "name": "query"
   }

> <list_commands>[1]["name"]
"query"

> :commands = <list_commands>
> echo :commands[1]["name"]
   query
```

# Pipes

Pipes are used to connect the output of one command to the input of another. They are mostly useful
for commands that generate IO streams. Pipes are created using the `|` operator, for example:

```
> list_commands | send { "Type": "some_type::list_commands" }
```

See the '`pipes`' help topic for more information on what pipes can be used for.
