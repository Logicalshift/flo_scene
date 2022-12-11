use super::proc_macro::*;

///
/// Implements the `#[derive(TalkMessage)]` attribute
///
/// This attribute can be applied to types to automatically implement the `TalkMessageType` trait
///
#[proc_macro_derive(TalkMessage)]
pub fn derive_talk_message(struct_or_enum: TokenStream) -> TokenStream {
    unimplemented!()
}
