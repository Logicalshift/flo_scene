use super::derive_message_format::*;

use syn::*;
use proc_macro::{TokenStream};
use quote::{quote};

use std::env;

///
/// Creates the SceneMessage implementation for a type
///
pub (crate) fn generate_has_message_format(type_name: Ident, data: &Data) -> TokenStream {
    let prefix = if env::var("CARGO_PKG_NAME") == Ok("flo_scene".into()) {
        quote! { crate }
    } else {
        quote! { ::flo_scene }
    };

    // Generate the type data for this message
    let message_format_expr = message_format_expression(data);
    let message_format      = quote! { 
        fn message_format() -> Option<#prefix::message_format::MessageFormat> {
            use #prefix::message_format::*;

            #message_format_expr
        }
    };

    // Put together the scene message definition
    quote! {
        impl #prefix::SceneMessage for #type_name {
            #message_format
        }
    }.into()
}
