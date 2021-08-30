extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(LatticeRepr)]
pub fn my_macro(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    // Used in the quasi-quotation below as `#name`.
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Build the output, possibly using quasi-quotation.
    // TODO
    let expanded = quote! {
        impl #impl_generics ::hydroflow::lattice::LatticeRepr for #name #ty_generics #where_clause {
            fn heap_size_of_children(&self) -> usize {
                0
            }
        }
    };

    // Hand the output tokens back to the compiler.
    TokenStream::from(expanded)
}
