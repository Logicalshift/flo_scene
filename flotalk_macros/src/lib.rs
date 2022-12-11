#[macro_use] extern crate quote;

use proc_macro::{TokenStream};
use syn;
use syn::{Ident, Generics, Data, DataEnum, DataStruct};

///
/// Creates the code to implement TalkMessageType for an enum
///
fn derive_enum_message(name: &Ident, generics: &Generics, data: &DataEnum) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // An enum value like 'Int(i64)' is converted to a message 'withInt: 64'
    let talk_message_type = quote! {
        impl #impl_generics ::flo_talk::TalkMessageType for #name #where_clause {
            /// Converts a message to an object of this type
            fn from_message<'a>(message: ::flo_talk::TalkOwned<'a, ::flo_talk::TalkMessage>, context: &'a ::flo_talk::TalkContext) -> Result<Self, ::flo_talk::TalkError> {
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
