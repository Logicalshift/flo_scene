#[macro_use] extern crate quote;

use proc_macro::{TokenStream};
use proc_macro2;
use proc_macro2::{Span};
use syn;
use syn::{Ident, Generics, Data, DataEnum, DataStruct};

use once_cell::sync::{Lazy};
use std::sync::atomic::{AtomicU64, Ordering};

///
/// Creates a static value for a symbol with a unique ID
///
fn symbol_static(name: &str) -> (proc_macro2::TokenStream, Ident) {
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
fn message_signature_static(symbols: Vec<&str>) -> (proc_macro2::TokenStream, Ident) {
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
/// Creates the code to implement TalkMessageType for an enum
///
fn derive_enum_message(name: &Ident, generics: &Generics, data: &DataEnum) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let msg = message_signature_static(vec!["test1:", "test2:"]).0;

    // An enum value like 'Int(i64)' is converted to a message 'withInt: 64'
    let talk_message_type = quote! {
        impl #impl_generics ::flo_talk::TalkMessageType for #name #where_clause {
            /// Converts a message to an object of this type
            fn from_message<'a>(message: ::flo_talk::TalkOwned<'a, ::flo_talk::TalkMessage>, context: &'a ::flo_talk::TalkContext) -> Result<Self, ::flo_talk::TalkError> {
                #msg

                todo!()
            }

            /// Converts an object of this type to a message
            fn to_message<'a>(&self, context: &'a ::flo_talk::TalkContext) -> ::flo_talk::TalkOwned<'a, ::flo_talk::TalkMessage> {
                todo!()
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
