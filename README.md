# flo_scene

This crate profides a way to build large programs out of smaller interacting programs. A 'scene' is made up of a number of entities that each run a self-contained asynchronous program. Entities communicate with each other by sending messages and receiving responses.

This is quite similar to the idea of an 'Entity Component System', with the notable difference that entities communicate with message streams instead of by exposing properties. It's even more similar to the original intent behind the idea of 'object oriented design', where the key idea is objects that exchange messages rather than the more 'modern' notion of classes that inherit from one another. Rust is a language that is particularly well suited for this kind of design.


