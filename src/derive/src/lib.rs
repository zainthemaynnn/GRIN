use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use quote::{format_ident, quote};
use syn::{parse_macro_input, punctuated::Punctuated, spanned::Spanned, DeriveInput, Meta, Token};

#[proc_macro_derive(Cooldown, attributes(cooldown))]
pub fn derive_cooldown(input: TokenStream) -> TokenStream {
    match impls(parse_macro_input!(input as DeriveInput)) {
        Ok(stream) => stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn impls(input: DeriveInput) -> Result<proc_macro2::TokenStream, syn::Error> {
    let ident = &input.ident;
    let mut duration = None;

    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cooldown"))
    {
        for sub_attr in attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .unwrap()
        {
            match sub_attr {
                Meta::NameValue(meta) if meta.path.is_ident("duration") => {
                    duration = Some(meta.value);
                }
                _ => {
                    return Err(syn::Error::new(
                        sub_attr.span(),
                        "Unidentified meta attribute.",
                    ));
                }
            }
        }
    }

    let Some(duration) = duration else {
        return Err(syn::Error::new(ident.span(), "Missing cooldown duration."));
    };

    let bevy = get_crate("bevy");
    let grin_ai = get_crate("grin_ai");

    Ok(proc_macro2::TokenStream::from(quote! {
        impl Default for #ident {
            fn default() -> Self {
                Self(#bevy::prelude::Timer::from_seconds(#duration, #bevy::prelude::TimerMode::Repeating))
            }
        }

        impl #grin_ai::Cooldown for #ident {
            fn timer(&self) -> &#bevy::prelude::Timer {
                &self.0
            }

            fn timer_mut(&mut self) -> &mut #bevy::prelude::Timer {
                &mut self.0
            }
        }
    }))
}

fn get_crate(name: &str) -> proc_macro2::TokenStream {
    let found_crate = proc_macro_crate::crate_name(name)
        .expect(&format!("`{}` is present in `Cargo.toml`", name));

    match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = format_ident!("{}", &name);
            quote!( #ident )
        }
    }
}
