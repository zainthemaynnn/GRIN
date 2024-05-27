use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, punctuated::Punctuated, spanned::Spanned, Data, DeriveInput, Meta, Token,
};

#[proc_macro_derive(Cooldown, attributes(cooldown))]
pub fn derive_cooldown(input: TokenStream) -> TokenStream {
    match impl_cooldown(parse_macro_input!(input as DeriveInput)) {
        Ok(stream) => stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn impl_cooldown(input: DeriveInput) -> Result<proc_macro2::TokenStream, syn::Error> {
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

#[proc_macro_derive(Spawnable)]
pub fn derive_spawnable(input: TokenStream) -> TokenStream {
    match impl_spawnable(parse_macro_input!(input as DeriveInput)) {
        Ok(stream) => stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn impl_spawnable(input: DeriveInput) -> Result<proc_macro2::TokenStream, syn::Error> {
    let ident = &input.ident;
    let event_ident = format_ident!("{}SpawnEvent", input.ident);

    let bevy = get_crate("bevy");
    let grin_util = get_crate("grin_util");

    Ok(proc_macro2::TokenStream::from(quote! {
        #[derive(#bevy::prelude::Event, Clone, Default)]
        pub struct #event_ident {
            pub transform: Transform,
        }

        impl #grin_util::event::Spawnable for #ident {
            type Event = #event_ident;
        }
    }))
}

#[proc_macro_derive(TypedEvents)]
pub fn derive_typed_events(input: TokenStream) -> TokenStream {
    match impl_typed_events(parse_macro_input!(input as DeriveInput)) {
        Ok(stream) => stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn impl_typed_events(input: DeriveInput) -> Result<proc_macro2::TokenStream, syn::Error> {
    let ident = &input.ident;

    let grin_util = get_crate("grin_util");
    let bevy_enum_filter = get_crate("bevy_enum_filter");
    let Data::Enum(data_enum) = &input.data else {
        return Err(syn::Error::new(ident.span(), "Cannot derive for non-enum."));
    };
    let variants = data_enum.variants.iter().map(|var| &var.ident);

    Ok(proc_macro2::TokenStream::from(quote! {
        impl #ident {
            /// Converts an `UntypedEvent` to its typed counterpart, annotated with the enum filter struct
            /// for this variant.
            pub fn typed_event<E: #grin_util::event::UntypedEvent>(&self, ev: &E) -> E::TypedEvent<impl Component> {
                match self {
                    #( #ident::#variants => ev.typed::<#bevy_enum_filter::Enum!(#ident::#variants)>() ),*
                }
            }
        }
    }))
}

fn get_crate(name: &str) -> proc_macro2::TokenStream {
    let found_crate = proc_macro_crate::crate_name(name)
        .expect(&format!("`{}` is not present in `Cargo.toml`", name));

    match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = format_ident!("{}", &name);
            quote!( #ident )
        }
    }
}
