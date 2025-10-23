use super::derive_message_format::*;

use syn::*;
use proc_macro::{TokenStream};
use quote::{quote};

use std::env;

///
/// Possible default targets for a scene message
///
#[derive(Clone)]
pub enum SceneMessageDefaultTarget {
    None,
    Any,
    SubProgramCalled(String),
}

///
/// Description of the attributes 
///
pub struct SceneMessageAttributes {
    pub (crate) crate_name:             String,
    pub (crate) message_type_name:      String,
    pub (crate) not_serializable:       bool,
    pub (crate) default_target:         SceneMessageDefaultTarget,
    pub (crate) has_initialisation:     bool,
    pub (crate) allow_thread_stealing:  bool,
}

impl SceneMessageAttributes {
    ///
    /// Parses the message attributes from an AST for an enum or a struct
    ///
    pub fn from_ast(crate_name: &str, ast: &DeriveInput) -> Self {
        // Set up with the default values
        let mut attributes = Self {
            crate_name:             crate_name.into(),
            message_type_name:      Self::type_name_from_ast(ast),
            not_serializable:       false,
            default_target:         SceneMessageDefaultTarget::Any,
            has_initialisation:     false,
            allow_thread_stealing:  false,
        };

        // Read through the AST to discover the attributes the user might have set on the structure
        for attr in ast.attrs.iter() {
            if attr.path().is_ident("message_type_name") {
                attributes.set_message_type_name_from_attribute(attr);
            }

            if attr.path().is_ident("default_target") {
                attributes.set_default_target_from_attribute(attr);
            }

            if attr.path().is_ident("not_serializable") {
                attributes.not_serializable = true;
            }

            if attr.path().is_ident("has_initialisation") {
                attributes.has_initialisation = true;
            }

            if attr.path().is_ident("allow_thread_stealing_by_default") {
                attributes.allow_thread_stealing = true;
            }
        }

        attributes
    }

    ///
    /// Returns the structure/enum name that we're deriving from
    ///
    fn type_name_from_ast(ast: &DeriveInput) -> String {
        ast.ident.to_string()
    }

    ///
    /// Sets the name of the message type from a `#[message_type_name]` attribute
    ///
    fn set_message_type_name_from_attribute(&mut self, attribute: &Attribute) {
        // Parameter should be a list with one entry in it
        let Ok(message_name): Result<Ident> = attribute.parse_args() else { panic!("#[message_type_name()] should be used with a type name identifier (#[message_type_name(MyMessageType)])") };

        self.message_type_name = message_name.to_string();
    }

    ///
    /// Sets the default target subprogram name from a `#[default_target]` attribute
    ///
    fn set_default_target_from_attribute(&mut self, attribute: &Attribute) {
        self.default_target = if let Ok(args) = attribute.parse_args::<LitStr>() {
            SceneMessageDefaultTarget::SubProgramCalled(args.value())
        } else if let Ok(args) = attribute.parse_args::<Ident>() {
            if &args.to_string() == "None" {
                SceneMessageDefaultTarget::None
            } else if &args.to_string() == "Any" {
                SceneMessageDefaultTarget::Any
            } else {
                panic!("#[default_target] should be used with a string with the default subprogram name in it (#[default(target(\"my_crate::my_program\")])");
            }
        } else {
            panic!("#[default_target] should be used with a string with the default subprogram name in it (#[default(target(\"my_crate::my_program\")])");
        };
    }
}

///
/// Creates the SceneMessage implementation for a type
///
pub (crate) fn generate_scene_message(type_name: Ident, attributes: &SceneMessageAttributes, data: &Data) -> TokenStream {
    let prefix = if env::var("CARGO_PKG_NAME") == Ok("flo_scene".into()) {
        quote! { crate }
    } else {
        quote! { ::flo_scene }
    };

    // Start with the message type name
    let message_type_name = format!("{}::{}", attributes.crate_name, attributes.message_type_name);
    let message_type_name = quote! {
        fn message_type_name() -> String { #message_type_name.into() }
    };

    // If a default target is defined, point it at the appropriate subprogram ID
    let default_target = match attributes.default_target.clone() {
        SceneMessageDefaultTarget::SubProgramCalled(default_target_name) => {
            quote! { 
                fn default_target() -> #prefix::StreamTarget {
                    #prefix::StreamTarget::Program(#prefix::SubProgramId::called(#default_target_name))
                }
            }
        }

        SceneMessageDefaultTarget::Any => {
            quote! {
                fn default_target() -> #prefix::StreamTarget {
                    #prefix::StreamTarget::Any
                }
            }
        }

        SceneMessageDefaultTarget::None => {
            quote! {
                fn default_target() -> #prefix::StreamTarget {
                    #prefix::StreamTarget::None
                }
            }
        }
    };

    // If the initialisation attribute is defined, we pass control on to a function defined in the 'impl' for the type
    let initialise = if attributes.has_initialisation {
        quote! {
            #[inline]
            fn initialise(context: &impl #prefix::SceneInitialisationContext) {
                Self::initialise_message(context);
            }
        }
    } else {
        quote! {
            #[inline]
            fn initialise(_: &impl #prefix::SceneInitialisationContext) { }
        }
    };

    // If the 'not serializable' attribute is set, return false from 'serializable' and also implement the serde serialization/deserialization structs with a dummy implementation
    let serializable = if attributes.not_serializable {
        quote! {
            fn serializable() -> bool { false }
        }
    } else {
        quote! {
            fn serializable() -> bool { true }
        }
    };

    let serialization_traits = if attributes.not_serializable {
        let error = format!("{} cannot be serialized", type_name);

        quote! {
            impl ::serde::Serialize for #type_name {
                fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: ::serde::Serializer 
                {
                    use serde::ser::{Error as SeError};
                    Err(S::Error::custom(#error))
                }
            }

            impl<'a> ::serde::Deserialize<'a> for #type_name {
                fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
                where
                    D: ::serde::Deserializer<'a> 
                {
                    use serde::de::{Error as DeError};
                    Err(D::Error::custom(#error))
                }
            }
        }
    } else {
        quote! {

        }
    };

    // Set the 'allow thread stealing' flag if set in the attributes
    let thread_stealing = if attributes.allow_thread_stealing {
        quote! {
            fn allow_thread_stealing_by_default() -> bool { true }
        }
    } else {
        quote! {
            fn allow_thread_stealing_by_default() -> bool { false }
        }
    };

    // Generat the type data for this message
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
            #default_target
            #initialise
            #thread_stealing
            #serializable
            #message_format
            #message_type_name
        }

        #serialization_traits
    }.into()
}
