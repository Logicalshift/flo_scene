#[macro_use] extern crate quote;

use proc_macro::{TokenStream};
use proc_macro2::{TokenStream as TokenStream2};
use proc_macro2::{Span};
use syn;
use syn::{Ident, Generics, Data, DataEnum, DataStruct, Variant, Fields, Field, Attribute};
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
fn signature_for_fields(parent_name: &Ident, fields: &Fields, attributes: Option<&Vec<Attribute>>) -> Vec<String> {
    let num_fields = fields.len();

    // Try to derive the name from the attributes
    for attr in attributes.iter().flat_map(|attrs| attrs.iter()) {
        if attr.path.is_ident("message") {
            let signature       = decode_message(attr);
            let signature_len   = if signature.len() == 1 && !signature[0].ends_with(":") { 0 } else { signature.len() };

            if signature_len != num_fields {
                // TODO: error handling
                panic!("#[message()] attribute used on a variant with {} fields, but there are {} fields in the supplied signature", num_fields, signature_len);
            }

            return signature;
        }
    }

    // Derive the name from the fields
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
    let signature                       = signature_for_fields(&variant.ident, &variant.fields, Some(&variant.attrs));
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
/// Creates a match arm for 'to_message' for a struct
///
fn struct_to_message(name: &Ident, data_struct: &DataStruct) -> TokenStream2 {
    // Get the signature
    let signature                       = signature_for_fields(name, &data_struct.fields, None);
    let (signature, signature_ident)    = message_signature_static(signature);

    // We call the values in the fields v0, v1, v2, etc
    let field_count;
    let match_fields = match &data_struct.fields {
        Fields::Named(named_fields) => {
            field_count = named_fields.named.len();
            let fields  = named_fields.named.iter().enumerate()
                .map(|(idx, field)| {
                    let field_ident = field.ident.as_ref().expect("Fields to have a name");
                    let value_ident = Ident::new(&format!("v{}", idx), field.span());
                    quote_spanned! { field.span() => #field_ident: #value_ident }
                });

            quote_spanned! { data_struct.fields.span() => #name { #(#fields),* } }
        }

        Fields::Unnamed(unnamed_fields) => { 
            field_count     = unnamed_fields.unnamed.len();
            let field_names = (0..field_count).into_iter().map(|idx| Ident::new(&format!("v{}", idx), data_struct.fields.span()));

            quote_spanned! { data_struct.fields.span() => #name(#(#field_names),*) }
        }

        Fields::Unit => {
            field_count = 0;
            quote_spanned! { data_struct.fields.span() => #name }
        }
    };

    // Create the match arm
    if field_count == 0 {
        // Unary message
        quote_spanned! { data_struct.fields.span() =>
            #signature

            ::flo_talk::TalkMessage::Unary(*#signature_ident)
        }
    } else {
        // Multiple-argument message
        let field_names = (0..field_count).into_iter().map(|idx| Ident::new(&format!("v{}", idx), data_struct.fields.span()));

        quote_spanned! { data_struct.fields.span() =>
            #signature
            let #match_fields = self;
            let message       = ::flo_talk::TalkMessage::WithArguments(*#signature_ident, smallvec![#(#field_names.into_talk_value(context).leak()),*]);
         }
    }
}

///
/// Creates a match arm for a 'from_message' for an enum variant
///
/// Expects 'signature_id' to contain the MessageSignatureId and '_args' to contain the arguments
///
fn enum_variant_from_message(name: &Ident, variant: &Variant) -> TokenStream2 {
    // Convert the enum variant to a signature, and store that signature in a static variable
    let signature                       = signature_for_fields(&variant.ident, &variant.fields, Some(&variant.attrs));
    let (signature, signature_ident)    = message_signature_static(signature);

    let variant_name                    = &variant.ident;

    // Create the code to construct this variant
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
/// Creates a match arm for a 'from_message' for an enum variant, with alternate matching
///
/// Expects '_first_symbol' to contain the initial symbol for the message, and '_args' to contain the arguments, returns None if there
/// are no alternatives
///
fn enum_variant_from_message_alternate(name: &Ident, variant: &Variant) -> Option<TokenStream2> {
    // For 'unnamed' enum variants, we also support any message where the first symbol matches (so you can give these messages any name)
    let signature                       = signature_for_fields(&variant.ident, &variant.fields, Some(&variant.attrs));
    if signature.len() < 2 { return None; }
    let (symbol, symbol_ident)          = symbol_static(&signature[0]);

    let variant_name                    = &variant.ident;

    let create_variant = match &variant.fields {
        Fields::Named(_)    => { return None; }
        Fields::Unit        => { return None; }

        Fields::Unnamed(unnamed_fields) => {
            if unnamed_fields.unnamed.len() == 0 {
                return None;
            }

            // Convert each field to its own type from a talk value (taken from _args)
            let fields = unnamed_fields.unnamed.iter().enumerate()
                .map(|(arg_num, field)| {
                    let ty = &field.ty;
                    quote_spanned! { field.span() => #ty::try_from_talk_value(TalkOwned::new(_args[#arg_num].take(), _context), _context)? }
                });

            // Return result is from converting all the arguments
            quote_spanned! { variant.span() => Ok(#name::#variant_name(#(#fields),*)) }
        }
    };

    // Match against this signature ID and return the result of creating the value if it does match
    Some(quote_spanned! { variant.span() =>
        #symbol
        if _first_symbol == *#symbol_ident {
            let mut _args = _args.unwrap();
            return #create_variant;
        }
    })
}

///
/// Creates a match arm for a 'from_message' for a structure
///
/// Expects 'signature_id' to contain the MessageSignatureId and '_args' to contain the arguments
///
fn struct_from_message(name: &Ident, data_struct: &DataStruct) -> TokenStream2 {
    // Convert the structure to a signature, and store that signature in a static variable
    let signature                       = signature_for_fields(name, &data_struct.fields, None);
    let (signature, signature_ident)    = message_signature_static(signature);

    // Create the code to construct this structure
    let has_args;
    let create_struct = match &data_struct.fields {
        Fields::Named(named_fields) => { 
            has_args = named_fields.named.len() > 0;

            // Convert each field to its own type from a talk value (taken from _args)
            let fields = named_fields.named.iter().enumerate()
                .map(|(arg_num, field)| {
                    let ty          = &field.ty;
                    let field_ident = field.ident.as_ref().expect("Fields to have a name");
                    quote_spanned! { field.span() => #field_ident: #ty::try_from_talk_value(TalkOwned::new(_args[#arg_num].take(), _context), _context)? }
                });

            quote_spanned! { data_struct.fields.span() => Ok(#name { #(#fields),* }) }
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
            quote_spanned! { data_struct.fields.span() => Ok(#name(#(#fields),*)) }
        }

        Fields::Unit => {
            has_args = false;
            quote_spanned! { data_struct.fields.span() => Ok(#name) }
        }
    };

    // Match against this signature ID and return the result of creating the value if it does match
    if has_args {
        quote_spanned! { data_struct.fields.span() =>
            #signature
            if *signature_id == *#signature_ident {
                let mut _args = _args.unwrap();
                return #create_struct;
            }
        }
    } else {
        quote_spanned! { data_struct.fields.span() =>
            #signature
            if *signature_id == *#signature_ident {
                return #create_struct;
            }
        }
    }
}

///
/// Creates a match arm for a 'from_message' for a structure, with alternate matching
///
/// Expects '_first_symbol' to contain the initial symbol for the message, and '_args' to contain the arguments, returns None if there
/// are no alternatives
///
fn struct_from_message_alternate(name: &Ident, data_struct: &DataStruct) -> Option<TokenStream2> {
    // For 'unnamed' structures, we also support any message where the first symbol matches (so you can give these messages any name)
    let signature                       = signature_for_fields(name, &data_struct.fields, None);
    if signature.len() < 2 { return None; }
    let (symbol, symbol_ident)          = symbol_static(&signature[0]);

    let create_struct = match &data_struct.fields {
        Fields::Named(_)    => { return None; }
        Fields::Unit        => { return None; }

        Fields::Unnamed(unnamed_fields) => {
            if unnamed_fields.unnamed.len() == 0 {
                return None;
            }

            // Convert each field to its own type from a talk value (taken from _args)
            let fields = unnamed_fields.unnamed.iter().enumerate()
                .map(|(arg_num, field)| {
                    let ty = &field.ty;
                    quote_spanned! { field.span() => #ty::try_from_talk_value(TalkOwned::new(_args[#arg_num].take(), _context), _context)? }
                });

            // Return result is from converting all the arguments
            quote_spanned! { data_struct.fields.span() => Ok(#name(#(#fields),*)) }
        }
    };

    // Match against this signature ID and return the result of creating the value if it does match
    Some(quote_spanned! { data_struct.fields.span() =>
        #symbol
        if _first_symbol == *#symbol_ident {
            let mut _args = _args.unwrap();
            return #create_struct;
        }
    })
}

///
/// Creates the code to implement TalkMessageType and TalkValueType for an enum
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

    // Create match arms for each variants for the 'from_message' call
    let from_message_variant_arms = data.variants.iter()
        .flat_map(|variant| enum_variant_from_message_alternate(name, variant))
        .collect::<Vec<_>>();

    // An enum value like 'Int(i64)' is converted to a message 'withInt: 64'
    let talk_message_type = quote! {
        impl #impl_generics ::flo_talk::TalkMessageType for #name #ty_generics #where_clause {
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

                let _first_symbol = signature_id.to_signature().first_symbol();

                #(#from_message_variant_arms)*

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
        impl #impl_generics ::flo_talk::TalkValueType for #name #ty_generics #where_clause {
            fn into_talk_value<'a>(&self, context: &'a ::flo_talk::TalkContext) -> ::flo_talk::TalkOwned<'a, ::flo_talk::TalkValue> {
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
/// Creates the code to implement TalkMessageType and TalkValueType for a struct
///
fn derive_struct_message(name: &Ident, generics: &Generics, data: &DataStruct) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Create the 'to_message' call
    let to_message = struct_to_message(name, data);

    // Create the main 'from_message' call
    let from_message = struct_from_message(name, data);

    // Create 0 or 1 alternative matching style (depends on the struct)
    let from_message_alternates = struct_from_message_alternate(name, data).into_iter().collect::<Vec<_>>();

    // A struct like `struct Foo { bar: i64 baz: i64}` is converted to a message `withFooBar:baz:`
    let talk_message_type = quote! {
        impl #impl_generics ::flo_talk::TalkMessageType for #name #ty_generics #where_clause {
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

                #from_message

                let _first_symbol = signature_id.to_signature().first_symbol();

                #(#from_message_alternates)*

                Err(TalkError::MessageNotSupported(*signature_id))
            }

            /// Converts an object of this type to a message
            fn to_message<'a>(&self, context: &'a ::flo_talk::TalkContext) -> ::flo_talk::TalkOwned<'a, ::flo_talk::TalkMessage> {
                use ::flo_talk::smallvec::*;
                use ::flo_talk::{TalkValueType};

                #to_message

                TalkOwned::new(message, context)
            }
        }
    };

    // We also implement the TalkValueType trait for things that can be messages (they create message objects)
    let talk_value_type = if data.fields.len() == 1 {
        // Structs with one field can be decoded as a message or as a single value, and are preferentially encoded as a single value
        let (fetch_field, field_ty, create_struct) = match &data.fields {
            Fields::Named(named_fields)  => {
                let field_name      = named_fields.named[0].ident.as_ref().expect("Fields to have a name");
                let ty              = &named_fields.named[0].ty;
                let fetch_field     = quote! { self.#field_name };
                let create_struct   = quote! { #name { #field_name: field_value }};

                (fetch_field, ty, create_struct)
            }

            Fields::Unnamed(unnamed_fields) => {
                let ty              = &unnamed_fields.unnamed[0].ty;
                let fetch_field     = quote! { self.0 };
                let create_struct   = quote! { #name(field_value) };

                (fetch_field, ty, create_struct)
            }

            Fields::Unit => { unreachable!() }
        };

        quote! {
            impl #impl_generics ::flo_talk::TalkValueType for #name #ty_generics #where_clause {
                #[inline]
                fn into_talk_value<'a>(&self, context: &'a ::flo_talk::TalkContext) -> ::flo_talk::TalkOwned<'a, ::flo_talk::TalkValue> {
                    #fetch_field.into_talk_value(context)
                }

                fn try_from_talk_value<'a>(value: ::flo_talk::TalkOwned<'a, TalkValue>, context: &'a ::flo_talk::TalkContext) -> Result<Self, ::flo_talk::TalkError> {
                    if let Ok(field_value) = #field_ty::try_from_talk_value(TalkOwned::new(value.clone_in_context(context), context), context) {
                        Ok(#create_struct)
                    } else {
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
            }
        }
    } else { 
        // Structs with more than one field can only be decoded as a message
        quote! {
            impl #impl_generics ::flo_talk::TalkValueType for #name #ty_generics #where_clause {
                fn into_talk_value<'a>(&self, context: &'a ::flo_talk::TalkContext) -> ::flo_talk::TalkOwned<'a, ::flo_talk::TalkValue> {
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
        }
    };

    // Final result is bopth items
    quote! {
        #talk_message_type
        #talk_value_type
    }.into()
}

///
/// Decodes the message from a 'message' attribute
///
fn decode_message(attribute: &Attribute) -> Vec<String> {
    // Message should be of the form #[message(foo)]
    let message_string: syn::LitStr = attribute.parse_args().unwrap();
    let message_string              = message_string.value();

    // Split into message components
    message_string.split_inclusive(':').map(|component| component.to_string()).collect()
}

///
/// Implements the `#[derive(TalkMessageType)]` attribute
///
/// This attribute can be applied to types to automatically implement the `TalkMessageType` and `TalkValueType` traits
///
#[proc_macro_derive(TalkMessageType, attributes(message))]
pub fn derive_talk_message(struct_or_enum: TokenStream) -> TokenStream {
    // Use syn to parse the tokens
    let struct_or_enum: syn::DeriveInput = syn::parse(struct_or_enum).unwrap();

    // Encode as a enum or a struct type (unions are not supported)
    match &struct_or_enum.data {
        Data::Struct(struct_data)   => derive_struct_message(&struct_or_enum.ident, &struct_or_enum.generics, struct_data),
        Data::Enum(enum_data)       => derive_enum_message(&struct_or_enum.ident, &struct_or_enum.generics, enum_data),
        Data::Union(_)              => panic!("Only structs or enums can have the TalkMessageType trait applied to them")
    }
}
