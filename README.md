
# flo_scene

flo_scene is a crate that provides a way to build larger software by connecting smaller sub-programs. A 'sub-program'
is a component that can receive messages from a single input stream and generate messages for one or more output
streams.

Streams and messages are a looser way of connecting two subsystems together than function calls. Subprograms can be
more focused on single pieces of functionality and can be built to be fully self-contained, making them very easy
to test, re-implement or repurpose.

This is very similar to the concept of microservices, but simplified to what is hopefully its most basic form, and is
also a version of the original concept of object-oriented programming as implemented in languages like Simula or 
SmallTalk.
