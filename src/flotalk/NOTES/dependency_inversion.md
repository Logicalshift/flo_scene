# Aka the 'logging problem'

So a major difficulty with contemporary programming standards is that every single object needs to know about low-level things 
like loggers. This is probably because of a confusion between the two 'DIs'. Dependency Injection is a framework that deals with
excessive interdependencies by trying to automate away the problems they cause. Dependency inversion is a way to stop those
problems occurring in the first place.

A traditional logging object has 'log to' semantics. You call `logger debug: 'Some message'` to send a message to the logger
object, which you need to know about. The logger will need to perform some tricks to know who sent the message (so the dependency
situation gets worse: not only does every object need a reference to the logger, it needs its own personal logger that happens
to know what it's for).

Dependency inversion suggests a 'log from' approach. That is, instead of objects sending messages to the logger, the logger is
sent objects and monitors them for messages. In Rust, this can be achieved by making it so that objects can be queried for a
stream of logging data, or in `flo_scene` they can lodge messages with a logging entity which loggers can query to generate
messages.

The 'log from' semantics means that objects don't need to know about their loggers, and the loggers can find out about their
objects without any tricks.

In FloTalk we can use some of the dynamic semantics to make 'streaming' protocols. This is probably good for general IO too,
though standardising on it will likely mean the usual SmallTalk IO structures can't be emulated (this is probably OK, they're
outdated).

A streaming protocol is implemented on every class and every instance of that class. By default, these messages do nothing.
For logging we might use something like this:

```SmallTalk
self logDebug: 'something'.
self logWarning: 'more important something'.
self logError: 'super important something'.
```

A logger can attach to these protocols by modifying the dispatch table at a class level. If it needs to monitor only specific
objects (for the full 'log from' experience), it can use the references to determine which object is generating the message
and filter based on that. This can also be used for general IO.

Need to combine with a `self drop` message to make it so that we can stop tracking items when they're freed.
