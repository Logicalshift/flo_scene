# connect

The connect command creates and updates connections between subprograms. This is one of the distinctive
features of flo_scene: the way that the components of a program are connected together is not determined
by the components themselves, and can be changed on the fly.

## Usage

The `connect` command takes an argument consisting of a JSON object:

```
connect {
    "source_program": <Connection>
    "target_program": <Connection>
    "stream_type_name": <Stream name>
}
```

The source and the target specify which subprograms should be connected, and the stream type name specifies
the message type that is being connected. This is the serialization name used in the stream itself.

A `<Connection>` value is an enum that can have one of three value types:

|   |   |
| - | - |
| "None" | For the source, indicates that no connection is actually being made. For the target, indicates that any messages sent to the stream should be discarded |
| "Any" | For the source, indicates that all sources of this type should use this connection. For the target, indicates that the default target should be used, or that the stream should block until a specific target is chosen |
| { "Program": { "Named": "ProgramName" } } | The source or target should be the program called `ProgramName` |
| { "Program": { "Guid": "..." } } | The source or target should be a program named by a GUID |

## Examples

```
connect {
    "source_program": "Any",
    "target_program": { "Program": { "Named": "flo_scene::stdout" } },
    "stream_type_name": "flo_scene::TextOutput"
}
```

Connects all `TextOutput` streams to the standard output stream (this re-establishes the default connection)
