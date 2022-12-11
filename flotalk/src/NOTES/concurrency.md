# FloTalk and concurrency

## Contexts

A context represents a FloTalk runtime on a single thread: it is `Send` but not `Sync`. Multi-threaded concurrency can be achieved
using a class that communicates with another context.

The original idea here was to prevent the allocators from needing to use an `Arc<Mutex<...>>` to store their interior data. However
this is still needed by the current design as the allocator needs to be shared between several callback functions, and Rust can't
guarantee that they'll be sent between threads as a group. (Perhaps a trait instead of a group of functions might provide a way
around this)

## Asynchronous functions

FloTalk is intended for 'high-level' programming where performance isn't a huge concern (idea is scripts that generate simple instructions
that produce a lot of work for a Rust component), so there's no need for 'coloured' functions. Everything returns a continuation.

This makes it possible to run any function call asynchronously, which is quite helpful as a high-level thing, but creates some possible
synchronisation problems. The intention is to fix this by making it so that objects are assigned to an execution context: when a message
is sent, it is either sent directly to the object if it's in the current context or available to be reassigned, or it's added to a queue
for that object so that it's executed in the order that it occurs (a queue prevents the issue that can occur with simple locks where newer
tasks can cause older tasks to get deprioritised).

(Ideas: want a 'try' variant and a way to create backpressure)
