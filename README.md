
# flo_scene

flo_scene is a crate that provides a way to build large pieces of software by combining smaller
'sub-programs' that communicate via messages. This simplifies dependencies over more traditional
object-oriented techniques, as sub-programs do not need to directly depend on each other. There
are also benefits in terms of testing, code re-use and configurability.

## Basic usage

Scenes are created using `Scene::default()` or `Scene::empty()`. The empty scene contains no
subprograms by default but the default scene contains some default ones, in particular a
control program that can be used to start other programs or define connections between programs.

```Rust
use flo_scene::*;

let scene = Scene::default();
```

Sub-programs read from a single input stream of messages and can write to any number of output
streams. Each output stream can be connected to the input of another program, and these connections
can be specified independently of the programs themselves. Messages need to implement the 
`SceneMessage` trait, and subprograms can be added to a scene using the `add_subprogram()` function.

```Rust
// Simple logger
pub enum LogMessage {
    Info(String),
    Warning(String),
    Error(String)
}

impl SceneMessage for LogMessage { }

let log_program = SubProgramId::called("Logger");
scene.add_subprogram(log_program,
    |mut log_messages: InputStream<LogMessage>, _context| async move {
        while let Some(log_message) = log_messages.next().await {
            match log_message {
                LogMessage::Info(msg)       => { println!("INFO:    {:?}", msg); }
                LogMessage::Warning(msg)    => { println!("WARNING: {:?}", msg); }
                LogMessage::Error(msg)      => { println!("ERROR:   {:?}", msg); }
            }
        }
    },
    10)
```

Connections can be defined by the `connect_programs()` function. Note how this means that something
that generates log messages does not need to know their destination and that it's possible to change
how something is logging at run-time if needed.

```Rust
// Connect any program that writes log messages to our log program
// '()' means 'any source' here, it's possible to define connections on a per-subprogram basis if needed.
scene.connect_programs((), log_program, StreamId::with_message_type::<LogMessage>()).unwrap();
```

Subprograms have a context that can be used to retrieve output streams, or send single messages, so
after this connection is set up, anything can send log messages.

```Rust
let test_program = SubProgramId::new();
scene.add_subprogram(test_program,
    |_: InputStream<()>, context| async move {
        // '()' means send to any target
        let mut logger = context.send::<LogMessage>(());

        // Will send to the logger program
        logger.send(LogMessage::Warning("Hello".to_string())).await.unwrap();
    },
    0);
```

Once set up, the scene needs to be run, in the async context of your choice:

```Rust
executor::block_on(async {
    scene.run_scene().await;
});
```

A single scene can be run in multiple threads if needed and subprograms are naturally able to run
asynchronously as they communicate with messages rather than by direct data access.

## A few more advanced things

When the scene is created with `Scene::default()`, a control program is present that allows
subprograms to start other subprograms or create connections:

```Rust
    /* ... */
    context.send_message(SceneControl::start_program(new_program_id, |input, context| /* ... */, 10)).await.unwrap();
    context.send_message(SceneControl::connect(some_program, some_other_program, StreamId::for_message_type::<MyMessage>())).await.unwrap();
```

The empty scene does not get this control program (it's possible to use the `Scene` struct directly though).

Filters make it possible to connect two subprograms that take different message types by transforming
them. They need to be registered, then they can be used as a stream target:

```Rust
// Note that the stream ID identifies the *source* stream: there's only one input stream for any program
let mine_to_yours_filter = FilterHandle::for_filter(|my_messages: InputStream<MyMessage>| my_messages.map(|msg| YourMessage::from(msg)));
scene.connect(my_program, StreamTarget::Filtered(mine_to_yours_filter, your_program), StreamId::with_message_type::<MyMessage>()).unwrap();
```

The message type has some functions that can be overridden to provide some default behaviour which
can remove the need to manually configure connections whenever a scene is created:

```Rust
impl SceneMessage for MyMessage {
    fn default_target() -> StreamTarget {
        StreamTarget::Program(SubProgramId::called("MyMessageHandler"))
    }
}
```

A program can 'upgrade' its input stream to annotate the messages with their source if it needs this information:

```Rust
scene.add_subprogram(log_program,
    |log_messages: InputStream<LogMessage>, _context| async move {
        let mut log_messages = log_messages.messages_with_sources();
        while let Some((source, log_message)) = log_messages.next().await {
            match log_message {
                LogMessage::Info(msg)       => { println!("INFO:    [{:?}] {:?}", source, msg); }
                LogMessage::Warning(msg)    => { println!("WARNING: [{:?}] {:?}", source, msg); }
                LogMessage::Error(msg)      => { println!("ERROR:   [{:?}] {:?}", source, msg); }
            }
        }
    },
    10)
```

It is possible to request a stream directly to a particular program:

```Rust
    let mut logger = context.send::<LogMessage>(SubProgramId::called("MoreSpecificLogger"));
```

But it's also possible to redirect these with a connection request:

```Rust
// The scene is the ultimate arbiter of who can talk to who, so if we don't want our program talking to the MoreSpecificLogger after all we can change that
// Take care as this can get confusing!
scene.connect(exception_program, standard_logger_program, StreamId::for_target::<LogMessage>(SubProgramId::called("MoreSpecificLogger")));
```

You can create and run more than one `Scene` at once if needed.

## Philosophy

The core idea behind flo_scene is that there are at least three levels of abstraction in large
programs and these have different needs, and the last of these ('system') is not well-served
by the design of most programming languages:

 * Algorithms
 * Functions
 * Systems

Algorithms are basic code: they have full access to a lot of state, but they generally are bad at
re-using a part of themselves in different contexts.

Functions provide a way to solve the issue with algorithms. They allow an individual algorithm
to be used in many different contexts, and can provide a structural element to break an algorithm
down into simpler parts. They are limited to a single output given multiple inputs, and they tend
to get more nested as a piece of software grows. For large enough software, this nesting becomes
a problem: 'higher-level' functions can encapsulate a huge portion of the software's functionality
and have huge networks on interdependencies.

Subprograms are flo_scene's approach to providing a system-level construct. They take a single input
and can generate multiple outputs and have a flat dependency structure instead of a nested one.
Dependencies are defined externally rather than internally and can even be changed at run-time.
This relieves the problem of ever-growing complexity by removing the need for direct dependencies.

Object-oriented languages were supposed to provide a structure to relieve this nesting problem too,
but their modern variants gave up messaging for method calls, viewing the type system as more 
important than the structural one. Method calls are just functions, so these languages typically
do not provide the extra level of structure needed to relieve the nesting problem, unlike earlier
examples such as Simula and SmallTalk.

## See also

The key concept here is `Dependency Inversion`. The problem with building large software by nesting
smaller pieces of software ever deeper is that the upper levels incorporate the entire functionality
of the lower levels and hence become enormously complicated, hard to understand and hard to change.
Dependency inversion is an often poorly understood and implemented way to break this cycle by 
reversing the nesting order (eg, 'the logger has a reference to everything' instead of 'everything 
has a reference to the logger'). flo_scene is perhaps a step beyond this by breaking the direct 
dependencies between components entirely and defining them externally.

The idea of objects that communicate with messages probably originates with the Simula language, with
SmallTalk and Objective-C being other languages that notably adopt this approach.

Erlang is a functional language with a process model concept where processes communicate using 
mailboxes. An Erlang process is quite similar to a Simula object. flo_scene's model is also quite
similar with a key differentiator being the ability to connect sub-programs that have no knowledge
of each other.

Microservices are another quite similar concept, though it's far more heavyweight with the processes
being entire web services by themselves. A flo_scene subprogram could be an entirely separate service 
as easily as it could be a co-routine in the same process, but no direct support is provided in the
core crate for this.

All of these systems can be seen as different ways to implement the actor model, though most actor
frameworks that exist focus on concurrency issues and making messages work like functions to present
a familiar programming model (with its familiar nesting dependencies) over providing a structural
element.
