extern crate proc_macro;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident, Data, DataStruct, Fields};
use proc_macro_crate::{crate_name, FoundCrate};

#[proc_macro_derive(LatticeRepr)]
pub fn derive_latticerepr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    // Run the macro internals.
    // Hand the output tokens back to the compiler.
    derive_latticerepr_internal(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive_latticerepr_internal(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name_latrepr = input.ident;
    let vis = input.vis;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let crate_hydroflow = crate_hydroflow();

    let name_lattice = syn::parse_str::<Ident>(&*format!("{}Lattice", name_latrepr))?;
    let name_repr    = syn::parse_str::<Ident>(&*format!("{}Repr", name_latrepr))?;

    let fields = match input.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields), .. }) => fields.named,
        _ => panic!("this derive macro only works on structs with named fields"), // TODO: allow tuple structs
    };
    let repr_fields = fields.iter().map(|f| {
        // Interpolation only works for variables, not arbitrary expressions.
        // That's why we need to move these fields into local variables first
        // (borrowing would also work though).
        let field_name = &f.ident;
        let field_ty = &f.ty;
        let field_vis = &f.vis;

        quote! {
            #field_vis #field_name: <#field_ty as #crate_hydroflow ::lattice::LatticeRepr>::Repr,
        }
    });
    let repr_fields_clone = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            #field_name: self.#field_name.clone(),
        }
    });

    // Build the output, using quasi-quotation.
    let expanded = quote! {
        // LATTICE TYPE.
        #[automatically_derived]
        #vis struct #name_lattice #impl_generics #where_clause {
            _phantom: std::marker::PhantomData #ty_generics, // TODO: #ty_generics is what we want... except without the triangles<>.
        }
        #[automatically_derived]
        impl #impl_generics #crate_hydroflow ::lattice::Lattice for #name_lattice #ty_generics #where_clause {}

        // "REAL" REPRESENTATION TYPE.
        #[automatically_derived]
        #vis struct #name_repr #impl_generics #where_clause {
            #(#repr_fields)*
        }
        impl #impl_generics Clone for #name_repr #ty_generics #where_clause {
            fn clone(&self) -> Self {
                Self {
                    #(#repr_fields_clone)*
                }
            }
        }

        // LAT+REPR TYPE (LatticeRepr trait impl).
        #[automatically_derived]
        impl #impl_generics #crate_hydroflow ::lattice::LatticeRepr for #name_latrepr #ty_generics #where_clause {
            type Lattice = #name_lattice #ty_generics;
            type Repr = #name_repr #ty_generics;

            fn run(_inst: &mut Self::Repr) {
            }
        }
    };

    Ok(expanded)
}


fn crate_hydroflow() -> proc_macro2::TokenStream {
    let found_crate = crate_name("hydroflow").expect("`hydroflow` is present in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => quote!( crate ),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            quote!( #ident )
        }
    }
}