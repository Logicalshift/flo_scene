use syn::*;
use proc_macro2::{TokenStream};
use quote::{quote};

///
/// Creates a 'message_format' expression for a Data definition (eg, a struct)
///
pub fn message_format_expression(data: &Data) -> TokenStream {
    match &data {
        Data::Struct(struct_defn) => {
            match &struct_defn.fields {
                Fields::Named(named_fields) => {
                    // Basic structure with named fields
                    let field_defns = named_fields.named.iter()
                        .map(format_field)
                        .collect::<Vec<_>>();

                    quote! {
                        Some(FormatDescriptor::Struct(vec![#(#field_defns),*]).into())
                    }
                }

                Fields::Unnamed(unnamed_fields) => {
                    quote! { None }
                }

                Fields::Unit => {
                    quote! { Some(FormatDescriptor::Tuple(vec![]).into()) }
                }
            }.into()
        }

        Data::Enum(enum_defn) => {
            // Enums are the most common basic message type
            let variants = enum_defn.variants.iter()
                .map(|variant| {
                    let variant_name    = variant.ident.to_string();
                    let argument_type   = match &variant.fields {
                        Fields::Named(named_fields) => {
                            // EnumVariant { struct_field: u8 }
                            let field_defns = named_fields.named.iter()
                                .map(format_field)
                                .collect::<Vec<_>>();

                            quote! { FormatDescriptor::Struct(vec![#(#field_defns),*]) }
                        }

                        Fields::Unnamed(unnamed_fields) => {
                            // EnumVariant(type, type, type)
                            let field_defns = unnamed_fields.unnamed.iter()
                                .map(|field_defn| format_type(&field_defn.ty))
                                .collect::<Vec<_>>();

                            quote! { FormatDescriptor::Tuple(vec![#(#field_defns),*]) }
                        }

                        Fields::Unit => {
                            // EnumVariant
                            quote! { FormatDescriptor::Tuple(vec![]).into() }
                        }
                    };

                    quote! { 
                        Variant {
                            name:          #variant_name,
                            argument_type: #argument_type,
                        }
                    }
                })
                .collect::<Vec<_>>();

            quote! {
                Some(FormatDescriptor::Enum(vec![#(#variants),*]))
            }.into()
        }

        Data::Union(_union_defn) => {
            quote! {
                None
            }
        }
    }
}

///
/// Formats a single field definition
///
fn format_field(field_defn: &Field) -> TokenStream {
    // We assume the field has a name
    let name        = field_defn.ident.as_ref().unwrap().to_string();
    let field_type  = format_type(&field_defn.ty);

    quote! {
        Field {
            name:       #name,
            field_type: #field_type
        }
    }
}

///
/// Formats a type definition
///
fn format_type(type_defn: &Type) -> TokenStream {
    match type_defn {
        Type::Path(type_path) => todo!("{:?}", type_defn),
        Type::Array(type_array) => todo!("{:?}", type_defn),
        Type::Tuple(type_tuple) => todo!("{:?}", type_defn),
        Type::Verbatim(token_stream) => todo!("{:?}", type_defn),
        Type::Slice(type_slice) => todo!("{:?}", type_defn),

        Type::BareFn(type_bare_fn) => todo!("{:?}", type_defn),
        Type::Group(type_group) => todo!("{:?}", type_defn),
        Type::ImplTrait(type_impl_trait) => todo!("{:?}", type_defn),
        Type::Infer(type_infer) => todo!("{:?}", type_defn),
        Type::Macro(type_macro) => todo!("{:?}", type_defn),
        Type::Never(type_never) => todo!("{:?}", type_defn),
        Type::Paren(type_paren) => todo!("{:?}", type_defn),
        Type::Ptr(type_ptr) => todo!("{:?}", type_defn),
        Type::Reference(type_reference) => todo!("{:?}", type_defn),
        Type::TraitObject(type_trait_object) => todo!("{:?}", type_defn),

        _ => unimplemented!("{:?}", type_defn),
    }
}