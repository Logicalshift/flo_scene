#[macro_use] extern crate quote;

use proc_macro::{TokenStream};
use proc_macro2::{TokenStream as TokenStream2};
use proc_macro2::{Span};
use syn;
use syn::{Ident, Generics, Data, DataEnum, DataStruct, Variant, Fields, Field};
use syn::spanned::Spanned;

use once_cell::sync::{Lazy};
use std::iter;
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
/// Returns the message parameter name to use for a named field
///
fn name_for_field(field: &Field) -> String {
    let field_ident = field.ident.as_ref().expect("Fields to have a name");
    field_ident.to_string()
}

///
/// Returns the message parameter name to use for a named field, with the first letter capitalised
///
fn capitalized_name_for_field(field: &Field) -> String {
    let lowercase       = name_for_field(field);
    let mut name_chrs   = lowercase.chars();

    if let Some(first_chr) = name_chrs.next() {
        first_chr.to_uppercase()
            .chain(name_chrs)
            .collect()
    } else {
        String::new()
    }
}

///
/// Creates the strings that make up the message signature for a list of fields
///
fn signature_for_fields(parent_name: &Ident, fields: &Fields) -> Vec<String> {
    match fields {
        Fields::Named(named_fields) => {
            if named_fields.named.len() == 0 {
                // If there are no fields, this works the same as a unit type
                vec![format!("with{}", parent_name.to_string())]
            } else {
                // If there are fields, we create a 'withStructuredFieldOne:fieldTwo:fieldThree:' type message signature
                let first_field     = format!("with{}{}:", parent_name.to_string(), capitalized_name_for_field(&named_fields.named[0]));
                let other_fields    = named_fields.named.iter().skip(1).map(|field| format!("{}:", name_for_field(field)));

                iter::once(first_field)
                    .chain(other_fields)
                    .collect()
            }
        }

        Fields::Unnamed(unnamed_fields) => {
            if unnamed_fields.unnamed.len() == 0 {
                // If there are no fields, this works the same as a unit type
                vec![format!("with{}", parent_name.to_string())]
            } else {
                // If there are fields, we create a 'withFoo:::' type message signature
                let first_field     = format!("with{}:", parent_name.to_string());
                let other_fields    = (1..(unnamed_fields.unnamed.len())).into_iter().map(|_| ":".to_string());

                iter::once(first_field)
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
        Fields::Named(named_fields) => {
            field_count = named_fields.named.len();
            let fields  = named_fields.named.iter().enumerate()
                .map(|(idx, field)| {
                    let field_ident = field.ident.as_ref().expect("Fields to have a name");
                    let value_ident = Ident::new(&format!("v{}", idx), field.span());
                    quote_spanned! { field.span() => #field_ident: #value_ident }
                });

            quote_spanned! { variant.span() => #name::#variant_name { #(#fields),* } }
        }

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

                ::flo_talk::TalkMessage::WithArguments(*#signature_ident, smallvec![#(#field_names.into_talk_value(context).leak()),*])
            }
         }
    }
}

///
/// Creates a match arm for a 'from_message' for an enum variant
///
fn enum_variant_from_message(name: &Ident, variant: &Variant) -> TokenStream2 {
    // Get the signature
    let signature                       = signature_for_fields(&variant.ident, &variant.fields);
    let (signature, signature_ident)    = message_signature_static(signature);

    let variant_name                    = &variant.ident;

    let has_args;
    let create_variant = match &variant.fields {
        Fields::Named(named_fields) => { 
            has_args = named_fields.named.len() > 0;

            // Convert each field to its own type from a talk value (taken from _args)
            let fields = named_fields.named.iter().enumerate()
                .map(|(arg_num, field)| {
                    let ty          = &field.ty;
                    let field_ident = field.ident.as_ref().expect("Fields to have a name");
                    quote_spanned! { field.span() => #field_ident: #ty::try_from_talk_value(TalkOwned::new(_args[#arg_num].take(), _context), _context)? }
                });

            quote_spanned! { variant.span() => Ok(#name::#variant_name { #(#fields),* }) }
        }

        Fields::Unnamed(unnamed_fields) => {
            has_args = unnamed_fields.unnamed.len() > 0;

            // Convert each field to its own type from a talk value (taken from _args)
            let fields = unnamed_fields.unnamed.iter().enumerate()
                .map(|(arg_num, field)| {
                    let ty = &field.ty;
                    quote_spanned! { field.span() => #ty::try_from_talk_value(TalkOwned::new(_args[#arg_num].take(), _context), _context)? }
                });

            // Return result is from converting all the arguments
            quote_spanned! { variant.span() => Ok(#name::#variant_name(#(#fields),*)) }
        }

        Fields::Unit => {
            has_args = false;
            quote_spanned! { variant.span() => Ok(#name::#variant_name) }
        }
    };

    // Match against this signature ID and return the result of creating the value if it does match
    if has_args {
        quote_spanned! { variant.span() =>
            #signature
            if *signature_id == *#signature_ident {
                let mut _args = _args.unwrap();
                return #create_variant;
            }
        }
    } else {
        quote_spanned! { variant.span() =>
            #signature
            if *signature_id == *#signature_ident {
                return #create_variant;
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

    // Create match arms for each variants for the 'from_message' call
    let from_message_arms = data.variants.iter()
        .map(|variant| enum_variant_from_message(name, variant))
        .collect::<Vec<_>>();

    // An enum value like 'Int(i64)' is converted to a message 'withInt: 64'
    let talk_message_type = quote! {
        impl #impl_generics ::flo_talk::TalkMessageType for #name #where_clause {
            /// Converts a message to an object of this type
            fn from_message<'a>(message: ::flo_talk::TalkOwned<'a, ::flo_talk::TalkMessage>, _context: &'a ::flo_talk::TalkContext) -> Result<Self, ::flo_talk::TalkError> {
                let mut message             = message;
                let (signature_id, _args)   = {
                    use ::flo_talk::smallvec::*;
                    use ::flo_talk::TalkMessage;

                    match &mut *message {
                        TalkMessage::Unary(sig)                 => (sig, None),
                        TalkMessage::WithArguments(sig, args)   => (sig, Some(args))
                    }
                };

                #(#from_message_arms)*

                Err(TalkError::MessageNotSupported(*signature_id))
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
            fn into_talk_value<'a>(self, context: &'a ::flo_talk::TalkContext) -> ::flo_talk::TalkOwned<'a, ::flo_talk::TalkValue> {
                use flo_talk::{TalkOwned, TalkValue};

                TalkOwned::new(TalkValue::Message(Box::new(self.to_message(context).leak())), context)
            }

            fn try_from_talk_value<'a>(value: ::flo_talk::TalkOwned<'a, TalkValue>, context: &'a ::flo_talk::TalkContext) -> Result<Self, ::flo_talk::TalkError> {
                let value = value.map(|val| {
                    match val {
                        ::flo_talk::TalkValue::Message(msg) => Some(*msg),
                        _                                   => { val.release_in_context(context); None }
                    }
                });

                match value.leak() {
                    Some(msg)   => Self::from_message(TalkOwned::new(msg, context), context),
                    None        => Err(TalkError::NotAMessage)
                }
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
        Data::Struct(struct_data)   => todo!("Structures"),
        Data::Enum(enum_data)       => derive_enum_message(&struct_or_enum.ident, &struct_or_enum.generics, enum_data),
        Data::Union(_)              => panic!("Only structs or enums can have the TalkMessageType trait applied to them")
    }
}
