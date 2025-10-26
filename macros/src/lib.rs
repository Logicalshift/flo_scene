mod scene_message_macro;
mod derive_message_format;
mod has_message_format_macro;

use syn::*;
use proc_macro::*;
use quote::{quote};

use std::env;

use scene_message_macro::*;
use has_message_format_macro::*;

///
/// 'derive' macro that will implement the `SceneMessage` trait on a type
///
#[proc_macro_derive(SceneMessage)]
pub fn scene_message_derive(input: TokenStream) -> TokenStream {
    // Parse the macro input
    let ast = parse_macro_input!(input as DeriveInput);

    // Crate name is used to generate a name for the message, if one isn't already present
    let crate_name = env::var("CARGO_PKG_NAME").unwrap();

    // Parse the attributes for the new scene message
    let type_name  = ast.ident.clone();
    let attributes = SceneMessageAttributes::from_ast(&crate_name, &ast);

    // Generate the scene message implementation
    let scene_message_defn      = generate_scene_message(type_name.clone(), &attributes, &ast.data);
    let has_message_format_defn = generate_has_message_format(type_name, &ast.data);

    quote! {
        #scene_message_defn
        #has_message_format_defn
    }.into()
}

///
/// 'derive' macro that will implement the `HasMessageFormat` trait on a type
///
/// Used for types that are used as part of messages but aren't messages themselves
///
#[proc_macro_derive(HasMessageFormat)]
pub fn has_message_format_derive(input: TokenStream) -> TokenStream {
    // Parse the macro input
    let ast = parse_macro_input!(input as DeriveInput);

    // Parse the attributes for the new scene message
    let type_name  = ast.ident.clone();

    // Generate the scene message implementation
    generate_has_message_format(type_name, &ast.data).into()
}

///
/// The `#[message_type_name(ThisMessageName)]` attribute defines the name that gets used for a scene message. The name of the
/// crate that the message is defined in is prepended to the message, so the generated message name
/// will be `my_crate::ThisMessageName`
///
#[proc_macro_attribute]
pub fn message_type_name(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Tokens are passed through unmodified
    item
}

///
/// `#[not_serializable]` can be added to a type that's using `#[derive(scene_message)]` to indicate that it's not a serializable
/// type. The message will return false from the `serializable()` method and implement dummy versions of the serde serialize and
/// deserialize traits.
///
#[proc_macro_attribute]
pub fn not_serializable(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

///
/// `#[default_target("target_subprogram_name")]` sets the default stream target of the message to be 
/// `SubProgramId::called("target_program_name")`
///
/// `#[default_target(None)]` can also be used to specify that the default target is `StreamTarget::None`,
/// indicating that these messages should be dropped by default.
///
/// `#[default_target(Any)]` is the default, indicating that `StreamTarget::Any` should be used, which
/// means that messages will be queued until a target becomes available by default.
///
#[proc_macro_attribute]
pub fn default_target(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

///
/// `#[has_initialisation]` causes the derived scene message to call the function `Self::initialise_message(&impl SceneInitialisationContext)`.
/// No initialisation is performed if this attribute is not present.
///
/// This is useful when a message has a default subprogram, or needs to set up some default filters.
///
#[proc_macro_attribute]
pub fn has_initialisation(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

///
/// `#[allow_thread_stealing_by_default]` sets the 'allow thread stealing' for any output stream sending this message. This causes
/// the target program to be invoked directly from the sending program rather than from a pass through the event loop, causing the
/// event to be delivered and processed much more quickly, at the cost of nesting in the stack and blocking the sender while the
/// message is processed.
///
/// This has several pitfalls so generally shouldn't be used, but can be useful for messages that need to be delivered with a high
/// priority or messages that need to be processed in a single-threaded context.
///
#[proc_macro_attribute]
pub fn allow_thread_stealing_by_default(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
