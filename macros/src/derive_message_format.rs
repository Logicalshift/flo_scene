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
                    let field_defns = named_fields.named.iter()
                        .map(format_field)
                        .collect::<Vec<_>>();

                    quote! {
                        Some(FormatDescriptor { Struct(vec![#(#field_defns),*]) }.into())
                    }
                }

                Fields::Unnamed(unnamed_fileds) => {
                    quote! { None }
                }

                Fields::Unit => {
                    quote! { Some(FormatDescriptor::Tuple(vec![]).into()) }
                }
            }.into()
        }

        Data::Enum(enum_defn) => {
            quote! {
                None
            }.into()
        }

        Data::Union(union_defn) => {
            todo!()
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
        Type::Array(type_array) => todo!(),
        Type::Tuple(type_tuple) => todo!(),
        Type::Verbatim(token_stream) => todo!(),
        Type::Slice(type_slice) => todo!(),

        Type::BareFn(type_bare_fn) => todo!(),
        Type::Group(type_group) => todo!(),
        Type::ImplTrait(type_impl_trait) => todo!(),
        Type::Infer(type_infer) => todo!(),
        Type::Macro(type_macro) => todo!(),
        Type::Never(type_never) => todo!(),
        Type::Paren(type_paren) => todo!(),
        Type::Path(type_path) => todo!(),
        Type::Ptr(type_ptr) => todo!(),
        Type::Reference(type_reference) => todo!(),
        Type::TraitObject(type_trait_object) => todo!(),

        _ => unimplemented!(),
    }
}