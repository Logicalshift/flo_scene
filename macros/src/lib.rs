mod scene_message_macro;

use syn::*;
use proc_macro::*;

use std::env;

use scene_message_macro::*;

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
    generate_scene_message(type_name, &attributes).into()
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