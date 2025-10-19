mod scene_message;

use proc_macro::*;
use quote::{quote};

///
/// 'derive' macro that will implement the `SceneMessage` trait on a type, along with the 'serialize' and 'deserialize' traits via serde
///
#[proc_macro_derive(SceneMessage)]
pub fn scene_message_derive(input: TokenStream) -> TokenStream {
    // Generate the scene message implementation
    let generated_code = quote! {
    };

    generated_code.into()
}
