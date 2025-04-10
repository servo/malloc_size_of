// Copyright 2016-2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A crate for deriving the MallocSizeOf trait.

use syn::parse_quote;
use synstructure::quote;

#[cfg(not(test))]
synstructure::decl_derive!([MallocSizeOf, attributes(ignore_malloc_size_of, conditional_malloc_size_of)] => malloc_size_of_derive);

fn malloc_size_of_derive(s: synstructure::Structure) -> proc_macro2::TokenStream {
    let match_body = s.each(|binding| {
        let mut ignore = false;
        let mut conditional = false;
        for attr in binding.ast().attrs.iter() {
            match attr.meta {
                syn::Meta::Path(ref path) | syn::Meta::List(syn::MetaList { ref path, .. }) => {
                    assert!(
                        !path.is_ident("ignore_malloc_size_of"),
                        "#[ignore_malloc_size_of] should have an explanation, \
                         e.g. #[ignore_malloc_size_of = \"because reasons\"]"
                    );
                    if path.is_ident("conditional_malloc_size_of") {
                        conditional = true;
                    }
                }
                syn::Meta::NameValue(syn::MetaNameValue { ref path, .. }) => {
                    if path.is_ident("ignore_malloc_size_of") {
                        ignore = true;
                    }
                    if path.is_ident("conditional_malloc_size_of") {
                        conditional = true;
                    }
                }
            }
        }

        assert!(
            !ignore || !conditional,
            "ignore_malloc_size_of and conditional_malloc_size_of are incompatible"
        );

        if ignore {
            return None;
        }

        let path = if conditional {
            quote! { ::malloc_size_of::MallocConditionalSizeOf::conditional_size_of }
        } else {
            quote! { ::malloc_size_of::MallocSizeOf::size_of }
        };

        if let syn::Type::Array(..) = binding.ast().ty {
            Some(quote! {
                for item in #binding.iter() {
                    sum += #path(item, ops);
                }
            })
        } else {
            Some(quote! {
                sum += #path(#binding, ops);
            })
        }
    });

    let ast = s.ast();
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let mut where_clause = where_clause.unwrap_or(&parse_quote!(where)).clone();
    for param in ast.generics.type_params() {
        let ident = &param.ident;
        where_clause
            .predicates
            .push(parse_quote!(#ident: ::malloc_size_of::MallocSizeOf));
    }

    let tokens = quote! {
        impl #impl_generics ::malloc_size_of::MallocSizeOf for #name #ty_generics #where_clause {
            #[inline]
            #[allow(unused_variables, unused_mut, unreachable_code)]
            fn size_of(&self, ops: &mut ::malloc_size_of::MallocSizeOfOps) -> usize {
                let mut sum = 0;
                match *self {
                    #match_body
                }
                sum
            }
        }
    };

    tokens
}

#[test]
fn test_struct() {
    let source = syn::parse_str(
        "struct Foo<T> { bar: Bar, baz: T, #[ignore_malloc_size_of = \"\"] z: Arc<T> }",
    )
    .unwrap();
    let source = synstructure::Structure::new(&source);

    let expanded = malloc_size_of_derive(source).to_string();
    let mut no_space = expanded.replace(" ", "");
    macro_rules! match_count {
        ($e: expr, $count: expr) => {
            assert_eq!(
                no_space.matches(&$e.replace(" ", "")).count(),
                $count,
                "counting occurrences of {:?} in {:?} (whitespace-insensitive)",
                $e,
                expanded
            )
        };
    }
    match_count!("struct", 0);
    match_count!("ignore_malloc_size_of", 0);
    match_count!("impl<T> ::malloc_size_of::MallocSizeOf for Foo<T> where T: ::malloc_size_of::MallocSizeOf {", 1);
    match_count!("sum += ::malloc_size_of::MallocSizeOf::size_of(", 2);

    let source = syn::parse_str("struct Bar([Baz; 3]);").unwrap();
    let source = synstructure::Structure::new(&source);
    let expanded = malloc_size_of_derive(source).to_string();
    no_space = expanded.replace(" ", "");
    match_count!("for item in", 1);
}

#[should_panic(expected = "should have an explanation")]
#[test]
fn test_no_reason() {
    let input = syn::parse_str("struct A { #[ignore_malloc_size_of] b: C }").unwrap();
    malloc_size_of_derive(synstructure::Structure::new(&input));
}
