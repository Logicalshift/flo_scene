//!
//! Raw test: this builds with `cargo build --target wasm32-unknown-unknown` and needs very few imports
//!
//! This gives us very light modules that aren't very capable, but if we're adding an ABI to communicate with flo_scene anyway
//! none of this matters, as these modules should only use the scene for communicating with the outside world. They're very
//! simple to load and run with wasmer, but probably need a bunch of hand-holding to work.
//!
//! These modules should work just as well in a browser as in wasmer.
//!

#[no_mangle]
pub fn test() -> u32 {
    42
}

#[repr(C)]
pub enum Foo {
    Foo(f32),
    Bar,
    Baz(u128),
}

#[no_mangle]
pub fn test2() -> Foo {
    Foo::Bar
}

#[repr(C)]
pub struct Bar {
    val1: i32,
    val2: f32,
}

#[no_mangle]
pub fn test3() -> Bar {
    Bar {
        val1: 42,
        val2: 120.2,
    }
}

#[no_mangle]
pub fn test4<'a>() -> &'a Bar {
    static BAR: Bar = Bar { val1: 42, val2: 120.2 };

    &BAR
}

#[no_mangle]
pub fn test5<'a>() -> &'a Foo {
    static FOO: Foo = Foo::Baz(42);

    &FOO
}

#[no_mangle]
pub extern fn test6() -> (i32, i32) {
    (42, 42)
}

#[no_mangle]
pub extern fn test7(closure: &dyn Fn(i32, i32) -> ()) {
    closure(42, 42)
}
