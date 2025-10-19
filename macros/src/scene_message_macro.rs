use syn::*;
use proc_macro::{TokenStream};
use quote::{quote};

///
/// Description of the attributes 
///
pub struct SceneMessageAttributes {
    pub (crate) crate_name:         String,
    pub (crate) message_type_name:  String,
    pub (crate) not_serializable:   bool,
    pub (crate) default_target:     Option<String>,
    pub (crate) has_initialisation: bool,
}

impl SceneMessageAttributes {
    ///
    /// Parses the message attributes from an AST for an enum or a struct
    ///
    pub fn from_ast(crate_name: &str, ast: &DeriveInput) -> Self {
        // Set up with the default values
        let mut attributes = Self {
            crate_name:         crate_name.into(),
            message_type_name:  Self::type_name_from_ast(ast),
            not_serializable:   false,
            default_target:     None,
            has_initialisation: false
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
        let Ok(args): Result<LitStr> = attribute.parse_args() else { panic!("#[default_target] should be used with a string with the default subprogram name in it (#[default(target(\"my_crate::my_program\")])"); };

        self.default_target = Some(args.value());
    }
}

///
/// Creates the SceneMessage implementation for a type
///
pub (crate) fn generate_scene_message(type_name: Ident, attributes: &SceneMessageAttributes) -> TokenStream {
    quote! {
        impl ::flo_scene::SceneMessage for #type_name {

        }
    }.into()
}
