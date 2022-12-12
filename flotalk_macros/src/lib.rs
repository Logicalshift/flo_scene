#[macro_use] extern crate quote;

use proc_macro::{TokenStream};
use proc_macro2::{TokenStream as TokenStream2};
use proc_macro2::{Span};
use syn;
use syn::{Ident, Generics, Data, DataEnum, DataStruct, Variant, Fields};
use syn::spanned::Spanned;

use once_cell::sync::{Lazy};
use std::sync::atomic::{AtomicU64, Ordering};

///
/// Creates a static value for a symbol with a unique ID
///
fn symbol_static(name: &str) -> (TokenStream2, Ident) {
    // All symbol types have a unique ID (we call them SYMBOL_x later on)
    static NEXT_SYMBOL_ID: Lazy<AtomicU64> = Lazy::new(|| { AtomicU64::new(0) });

    // Assign a new ID to this symbol
    let next_id = NEXT_SYMBOL_ID.fetch_add(1, Ordering::Relaxed);

    // Create an ident for this symbol
    let symbol_id = Ident::new(&format!("SYMBOL_{}", next_id), Span::call_site());

    // Create the declaration, using the version of once_cell linked from flo_talk
    let declaration = quote! { 
        static #symbol_id: ::flo_talk::once_cell::sync::Lazy<::flo_talk::TalkSymbol> = ::flo_talk::once_cell::sync::Lazy::new(|| ::flo_talk::TalkSymbol::from(#name));
    };

    (declaration.into(), symbol_id)
}

///
/// Creates a static value for a message signature
///
fn message_signature_static(symbols: Vec<String>) -> (TokenStream2, Ident) {
    // All symbol types have a unique ID (we call them MSG_SIG_X later on)
    static NEXT_MESSAGE_ID: Lazy<AtomicU64> = Lazy::new(|| { AtomicU64::new(0) });

    // Assign a new ID to this symbol
    let next_id = NEXT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed);

    // Create an ident for this symbol
    let symbol_id = Ident::new(&format!("MSG_SIG_{}", next_id), Span::call_site());

    // Create the declaration, using the version of once_cell linked from flo_talk
    let declaration = quote! { 
        static #symbol_id: ::flo_talk::once_cell::sync::Lazy<::flo_talk::TalkMessageSignatureId> = ::flo_talk::once_cell::sync::Lazy::new(|| 
            vec![
                #(::flo_talk::TalkSymbol::from(#symbols)),*
            ].into()
        );
    };

    (declaration.into(), symbol_id)
}

///
/// Creates the strings that make up the message signature for a list of fields
///
fn signature_for_fields(parent_name: &Ident, fields: &Fields) -> Vec<String> {
    match fields {
        Fields::Named(named_fields) => {
            todo!()
        }

        Fields::Unnamed(unnamed_fields) => {
            if unnamed_fields.unnamed.len() == 0 {
                // If there are no fields, this works the same as a unit type
                vec![format!("with{}", parent_name.to_string())]
            } else {
                // If there are fields, we create a 'withFoo:::' type message signature
                let first_field     = format!("with{}:", parent_name.to_string());
                let other_fields    = (1..(unnamed_fields.unnamed.len())).into_iter().map(|_| ":".to_string());

                vec![first_field].into_iter()
                    .chain(other_fields)
                    .collect()
            }
        }

        Fields::Unit => {
            vec![format!("with{}", parent_name.to_string())]
        }
    }
}

///
/// Creates a match arm for 'to_message' for an enum variant
///
fn enum_variant_to_message(name: &Ident, variant: &Variant) -> TokenStream2 {
    // Get the signature
    let signature                       = signature_for_fields(&variant.ident, &variant.fields);
    let (signature, signature_ident)    = message_signature_static(signature);

    let variant_name                    = &variant.ident;

    // We call the values in the fields v0, v1, v2, etc
    let field_count;
    let match_fields = match &variant.fields {
        Fields::Named(named_fields) => { todo!() }

        Fields::Unnamed(unnamed_fields) => { 
            field_count     = unnamed_fields.unnamed.len();
            let field_names = (0..field_count).into_iter().map(|idx| Ident::new(&format!("v{}", idx), variant.span()));

            quote_spanned! { variant.span() => #name::#variant_name(#(#field_names),*) }
        }

        Fields::Unit => {
            field_count = 0;
            quote_spanned! { variant.span() => #name::#variant_name }
        }
    };

    // Create the match arm
    if field_count == 0 {
        // Unary message
        quote_spanned! { variant.span() => #match_fields => {
                #signature

                ::flo_talk::TalkMessage::Unary(*#signature_ident)
            }
        }
    } else {
        // Multiple-argument message
        let field_names = (0..field_count).into_iter().map(|idx| Ident::new(&format!("v{}", idx), variant.span()));

        quote_spanned! { variant.span() => #match_fields => {
                #signature

                ::flo_talk::TalkMessage::WithArguments(*#signature_ident, smallvec![#(#field_names.try_into_talk_value(context).unwrap().leak()),*])
            }
         }
    }
}

///
/// Creates the code to implement TalkMessageType for an enum
///
fn derive_enum_message(name: &Ident, generics: &Generics, data: &DataEnum) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Create match arms for each variant for the 'to_message' call
    let to_message_arms = data.variants.iter()
        .map(|variant| enum_variant_to_message(name, variant))
        .collect::<Vec<_>>();

    // An enum value like 'Int(i64)' is converted to a message 'withInt: 64'
    let talk_message_type = quote! {
        impl #impl_generics ::flo_talk::TalkMessageType for #name #where_clause {
            /// Converts a message to an object of this type
            fn from_message<'a>(message: ::flo_talk::TalkOwned<'a, ::flo_talk::TalkMessage>, _context: &'a ::flo_talk::TalkContext) -> Result<Self, ::flo_talk::TalkError> {
                todo!()
            }

            /// Converts an object of this type to a message
            fn to_message<'a>(&self, context: &'a ::flo_talk::TalkContext) -> ::flo_talk::TalkOwned<'a, ::flo_talk::TalkMessage> {
                use ::flo_talk::smallvec::*;
                use ::flo_talk::{TalkValueType};

                let message = match self {
                    #(#to_message_arms),*
                };

                TalkOwned::new(message, context)
            }
        }
    };

    // We also implement the TalkValueType trait for things that can be messages (they create message objects)
    let talk_value_type = quote! {
        impl #impl_generics ::flo_talk::TalkValueType for #name #where_clause {
            fn try_into_talk_value<'a>(self, context: &'a ::flo_talk::TalkContext) -> Result<::flo_talk::TalkOwned<'a, ::flo_talk::TalkValue>, ::flo_talk::TalkError> {
                use flo_talk::{TalkOwned, TalkValue};

                Ok(TalkOwned::new(TalkValue::Message(Box::new(self.to_message(context).leak())), context))
            }

            fn try_from_talk_value<'a>(value: ::flo_talk::TalkOwned<'a, TalkValue>, context: &'a ::flo_talk::TalkContext) -> Result<Self, ::flo_talk::TalkError> {
                todo!()
            }
        }
    };

    // Final result is bopth items
    quote! {
        #talk_message_type
        #talk_value_type
    }.into()
}

///
/// Implements the `#[derive(TalkMessageType)]` attribute
///
/// This attribute can be applied to types to automatically implement the `TalkMessageType` and `TalkValueType` traits
///
#[proc_macro_derive(TalkMessageType)]
pub fn derive_talk_message(struct_or_enum: TokenStream) -> TokenStream {
    // Use syn to parse the tokens
    let struct_or_enum: syn::DeriveInput = syn::parse(struct_or_enum).unwrap();

    // Encode as a enum or a struct type (unions are not supported)
    match &struct_or_enum.data {
        Data::Struct(struct_data)   => todo!(),
        Data::Enum(enum_data)       => derive_enum_message(&struct_or_enum.ident, &struct_or_enum.generics, enum_data),
        Data::Union(_)              => panic!("Only structs or enums can have the TalkMessageType trait applied to them")
    }
}
