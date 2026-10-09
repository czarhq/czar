use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, GenericArgument, ItemFn, Pat, PathArguments, ReturnType, Type, parse_macro_input,
};

#[proc_macro_attribute]
pub fn task(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    expand_task(input).into()
}

fn expand_task(mut function: ItemFn) -> proc_macro2::TokenStream {
    if function.sig.asyncness.is_some() {
        return syn::Error::new_spanned(
            function.sig.fn_token,
            "#[task] does not support async fn yet",
        )
        .to_compile_error();
    }

    let original_ident = function.sig.ident.clone();
    let wrapper_ident = format_ident!("__czar_user_{}", original_ident);
    function.sig.ident = wrapper_ident.clone();

    let params: Vec<_> = function.sig.inputs.iter().collect();
    if params.is_empty() || params.len() > 2 {
        return syn::Error::new_spanned(
            &function.sig.inputs,
            "#[task] functions must accept `(input)` or `(input, ctx)`",
        )
        .to_compile_error();
    }

    let input_ty = match params[0] {
        FnArg::Typed(arg) => arg.ty.clone(),
        FnArg::Receiver(receiver) => {
            return syn::Error::new_spanned(receiver, "#[task] methods are not supported")
                .to_compile_error();
        }
    };

    let call_args = if params.len() == 2 {
        quote!(input, ctx)
    } else {
        quote!(input)
    };

    let output_ty = output_type(&function.sig.output);

    let return_stmt = match &function.sig.output {
        ReturnType::Default => {
            quote! {
                let output = match ::czar::into_task_result::<#output_ty, _>(#wrapper_ident(#call_args)) {
                    Ok(output) => output,
                    Err(err) => {
                        eprintln!("Task error: {}", err);
                        ::std::process::exit(1);
                    }
                };
            }
        }
        ReturnType::Type(_, _) => {
            quote! {
                let output = match ::czar::into_task_result::<#output_ty, _>(#wrapper_ident(#call_args)) {
                    Ok(output) => output,
                    Err(err) => {
                        eprintln!("Task error: {}", err);
                        ::std::process::exit(1);
                    }
                };
            }
        }
    };

    let mut input_binding = None;
    if let FnArg::Typed(arg) = params[0]
        && let Pat::Ident(ident) = arg.pat.as_ref()
    {
        input_binding = Some(ident.ident.clone());
    }
    let input_ident = input_binding.unwrap_or_else(|| format_ident!("input"));

    let ctx_binding = if params.len() == 2 {
        if let FnArg::Typed(arg) = params[1] {
            if let Pat::Ident(ident) = arg.pat.as_ref() {
                ident.ident.clone()
            } else {
                format_ident!("ctx")
            }
        } else {
            format_ident!("ctx")
        }
    } else {
        format_ident!("ctx")
    };

    quote! {
        #function

        fn main() {
            let #input_ident = match ::czar::read_input::<#input_ty>() {
                Ok(input) => input,
                Err(err) => {
                    eprintln!("Error reading input: {}", err);
                    ::std::process::exit(1);
                }
            };
            let #ctx_binding = ::czar::Context::from_env();
            #return_stmt
            if let Err(err) = ::czar::write_output(&output) {
                eprintln!("Error writing output: {}", err);
                ::std::process::exit(1);
            }
        }
    }
}

fn output_type(return_type: &ReturnType) -> Type {
    match return_type {
        ReturnType::Default => syn::parse_quote!(()),
        ReturnType::Type(_, ty) => extract_result_ok_type(ty).unwrap_or_else(|| *ty.clone()),
    }
}

fn extract_result_ok_type(ty: &Type) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match args.args.first()? {
        GenericArgument::Type(inner) => Some(inner.clone()),
        _ => None,
    }
}
