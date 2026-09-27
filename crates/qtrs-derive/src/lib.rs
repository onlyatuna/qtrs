use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, DeriveInput, Fields, Ident, LitStr, Type};

/// Derives `QObject` and generates a static `MetaObject` reflection descriptor (`QMetaObject`).
///
/// Automatically creates:
/// - `fn object_data(&self) -> &ObjectData`
/// - `fn object_data_mut(&mut self) -> &mut ObjectData`
/// - `fn as_qobject_any(&self) -> Option<&dyn Any>`
/// - `fn as_qobject_any_mut(&mut self) -> Option<&mut dyn Any>`
/// - `fn meta_object(&self) -> &'static MetaObject`
///
/// Supports field-level `#[property]` and `#[signal]` attributes:
/// ```ignore
/// #[derive(QObject)]
/// #[qobject(class_name = "MyButton")]
/// struct MyButton {
///     data: ObjectData,
///     #[property(notify = text_changed)]
///     text: String,
///     #[property(readonly)]
///     count: i32,
///     // A `Property<T>` field gets read/written through its own reactive engine
///     // (dirty-tracking, bindings, dependency graph) instead of a plain field write.
///     #[property]
///     hovered: Property<bool>,
///     #[signal]
///     clicked: Signal<()>,
///     #[signal]
///     text_changed: Signal<String>,
/// }
/// ```
/// Signal fields are dynamically invokable through `MetaObject`/`QObject::invoke_method`:
/// invoking one calls `.emit(..)` on the field after converting each `Variant` argument to
/// the signal's declared payload type(s).
///
/// `#[property(notify = some_signal)]` requires `some_signal` to be a `#[signal]` field
/// declared as `Signal<T>` where `T` matches the property's type (the `Property<T>`'s `T`,
/// or the field's own type for a plain field). The generated setter emits it whenever the
/// value actually changes, which additionally requires the property's type to implement
/// `PartialEq` (already required by `Property<T>` itself; a new, opt-in requirement for a
/// plain field only once `notify` is used on it).
/// Parsed `#[property(...)]` attribute arguments.
struct PropertyAttr {
    readonly: bool,
    notify: Option<Ident>,
}

/// A `#[property]` field resolved against its declared type and any `notify` target.
struct PropertyDesc {
    field: Ident,
    effective_type: Type,
    prop_name: String,
    readonly: bool,
    reactive: bool,
    notify_field: Option<Ident>,
}

#[proc_macro_derive(QObject, attributes(qobject, property, signal))]
pub fn derive_qobject(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    if !input.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &input.generics,
            "`#[derive(QObject)]` does not support generic structs",
        )
        .to_compile_error()
        .into();
    }
    if !matches!(
        &input.data,
        syn::Data::Struct(data) if matches!(&data.fields, Fields::Named(_))
    ) {
        return syn::Error::new_spanned(
            &input,
            "`#[derive(QObject)]` requires a struct with named fields",
        )
        .to_compile_error()
        .into();
    }
    let name = &input.ident;
    let name_str = name.to_string();

    // Check for class_name override in #[qobject(class_name = "...")]
    let mut class_name = name_str.clone();
    for attr in &input.attrs {
        if attr.path().is_ident("qobject") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("class_name") {
                    let s: LitStr = meta.value()?.parse()?;
                    class_name = s.value();
                }
                Ok(())
            });
        }
    }

    // Inspect fields for ObjectData and #[property]
    let mut data_field_ident: Option<Ident> = None;
    let mut raw_properties = Vec::new();

    let mut signals = Vec::new();

    if let syn::Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            for field in &fields_named.named {
                let f_ident = field.ident.clone().unwrap();
                let f_type = &field.ty;

                // Identify the ObjectData field (named `data` or with type ending in `ObjectData`)
                if f_ident == "data" || is_object_data_type(f_type) {
                    data_field_ident = Some(f_ident.clone());
                }

                // Check for #[property] attribute
                for attr in &field.attrs {
                    if attr.path().is_ident("property") {
                        let parsed = parse_property_attr(attr);
                        let prop_name = f_ident.to_string();
                        raw_properties.push((f_ident.clone(), f_type.clone(), prop_name, parsed));
                    }
                }

                for attr in &field.attrs {
                    if attr.path().is_ident("signal") {
                        if let Some(payload_types) = signal_payload_types(f_type) {
                            signals.push((f_ident.clone(), payload_types));
                        } else {
                            return syn::Error::new_spanned(
                                f_type,
                                "`#[signal]` fields must have type `Signal<T>`",
                            )
                            .to_compile_error()
                            .into();
                        }
                    }
                }
            }
        }
    }
    if data_field_ident.is_none() {
        return syn::Error::new_spanned(
            name,
            "`#[derive(QObject)]` requires a field named `data` or a field of type `ObjectData`",
        )
        .to_compile_error()
        .into();
    }

    // Resolve each #[property] field into a full descriptor: whether it's backed by a
    // `Property<T>` (routes through that reactive engine) or a plain field, its effective
    // (unwrapped) type for Variant conversion, and its validated NOTIFY signal, if any.
    let mut properties: Vec<PropertyDesc> = Vec::new();
    for (f_ident, f_type, prop_name, parsed) in &raw_properties {
        let reactive_inner = property_inner_type(f_type);
        let reactive = reactive_inner.is_some();
        let effective_type = reactive_inner.unwrap_or_else(|| f_type.clone());

        let notify_field = if let Some(notify_ident) = &parsed.notify {
            match signals
                .iter()
                .find(|(sig_ident, _)| sig_ident == notify_ident)
            {
                Some((_, payload_types)) if payload_types.len() == 1 => {
                    let declared_ty = &payload_types[0];
                    let declared = normalize_type_string(&quote!(#declared_ty).to_string());
                    let expected = normalize_type_string(&quote!(#effective_type).to_string());
                    if declared != expected {
                        return syn::Error::new_spanned(
                            notify_ident,
                            format!(
                                "`#[property(notify = {notify_ident})]` on `{f_ident}` requires \
                                 `#[signal] {notify_ident}: Signal<{expected}>`, found `Signal<{declared}>`"
                            ),
                        )
                        .to_compile_error()
                        .into();
                    }
                    Some(notify_ident.clone())
                }
                Some(_) => {
                    return syn::Error::new_spanned(
                        notify_ident,
                        format!(
                            "`#[property(notify = {notify_ident})]` on `{f_ident}` requires a \
                             single-parameter signal matching the property's type"
                        ),
                    )
                    .to_compile_error()
                    .into();
                }
                None => {
                    return syn::Error::new_spanned(
                        notify_ident,
                        format!(
                            "`#[property(notify = {notify_ident})]` on `{f_ident}` refers to no \
                             `#[signal]` field named `{notify_ident}`"
                        ),
                    )
                    .to_compile_error()
                    .into();
                }
            }
        } else {
            None
        };

        properties.push(PropertyDesc {
            field: f_ident.clone(),
            effective_type,
            prop_name: prop_name.clone(),
            readonly: parsed.readonly,
            reactive,
            notify_field,
        });
    }

    let (data_access, data_access_mut) = if let Some(field) = &data_field_ident {
        (quote! { &self.#field }, quote! { &mut self.#field })
    } else {
        (quote! { &self.data }, quote! { &mut self.data })
    };

    // Generate MetaProperty items
    let mut meta_properties = Vec::new();
    let meta_obj_ident = Ident::new(&format!("__{}_META_OBJECT", name), name.span());

    for desc in &properties {
        let getter_fn_ident =
            Ident::new(&format!("__qtrs_get_{}_{}", name, desc.field), name.span());
        let setter_fn_ident =
            Ident::new(&format!("__qtrs_set_{}_{}", name, desc.field), name.span());

        let (getter_impl, setter_impl) =
            generate_property_accessors(name, desc, &getter_fn_ident, &setter_fn_ident);

        meta_properties.push(quote! {
            #getter_impl
            #setter_impl
        });
    }

    let prop_descriptors: Vec<_> = properties
        .iter()
        .map(|desc| {
            let is_writable = !desc.readonly;
            let getter_fn_ident =
                Ident::new(&format!("__qtrs_get_{}_{}", name, desc.field), name.span());
            let setter_fn_ident =
                Ident::new(&format!("__qtrs_set_{}_{}", name, desc.field), name.span());
            let prop_name = &desc.prop_name;
            let effective_type = &desc.effective_type;

            let setter_ref = if is_writable {
                quote! { Some(#setter_fn_ident) }
            } else {
                quote! { None }
            };

            let notify_ref = if let Some(notify_field) = &desc.notify_field {
                let notify_name = notify_field.to_string();
                quote! { Some(#notify_name) }
            } else {
                quote! { None }
            };

            let type_name_str = quote!(#effective_type).to_string();

            quote! {
                ::qtrs_core::meta::MetaProperty::new(
                    #prop_name,
                    #type_name_str,
                    true,
                    #is_writable,
                    false,
                    false,
                    #notify_ref,
                    Some(#getter_fn_ident),
                    #setter_ref,
                )
            }
        })
        .collect();

    let meta_props_slice = quote! {
        &[ #(#prop_descriptors),* ]
    };

    let mut signal_invokers = Vec::new();
    let signal_descriptors: Vec<_> = signals
        .iter()
        .map(|(field, payload_types)| {
            let sig_name = field.to_string();
            let parameter_types: Vec<_> = payload_types
                .iter()
                .map(|ty| LitStr::new(&quote!(#ty).to_string().replace(' ', ""), field.span()))
                .collect();
            let signature = format!(
                "{}({})",
                sig_name,
                parameter_types
                    .iter()
                    .map(|ty| ty.value())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let signature = LitStr::new(&signature, field.span());

            let invoker_fn_ident =
                Ident::new(&format!("__qtrs_emit_{}_{}", name, field), name.span());
            signal_invokers.push(generate_signal_invoker(
                name,
                field,
                payload_types,
                &invoker_fn_ident,
            ));

            quote! {
                ::qtrs_core::meta::MetaMethod::new(
                    #sig_name,
                    #signature,
                    "()",
                    &[#(#parameter_types),*],
                    &[],
                    ::qtrs_core::meta::MethodType::Signal,
                    ::qtrs_core::meta::Access::Public,
                    Some(#invoker_fn_ident),
                )
            }
        })
        .collect();

    let expanded = quote! {
        #(#meta_properties)*
        #(#signal_invokers)*

        #[allow(non_upper_case_globals)]
        static #meta_obj_ident: ::qtrs_core::meta::MetaObject = ::qtrs_core::meta::MetaObject::new(
            #class_name,
            Some(&::qtrs_core::meta::QOBJECT_META_OBJECT),
            &[ #(#signal_descriptors),* ],
            #meta_props_slice,
            &[],
            &[],
        );

        impl ::qtrs_core::object::QObject for #name {
            fn object_data(&self) -> &::qtrs_core::object::ObjectData {
                #data_access
            }

            fn object_data_mut(&mut self) -> &mut ::qtrs_core::object::ObjectData {
                #data_access_mut
            }

            fn as_qobject_any(&self) -> Option<&dyn ::std::any::Any> {
                Some(self)
            }

            fn as_qobject_any_mut(&mut self) -> Option<&mut dyn ::std::any::Any> {
                Some(self)
            }

            fn meta_object(&self) -> &'static ::qtrs_core::meta::MetaObject {
                &#meta_obj_ident
            }
        }
    };

    TokenStream::from(expanded)
}

fn signal_payload_types(ty: &Type) -> Option<Vec<Type>> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Signal" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let mut generic_args = args.args.iter();
    let Some(syn::GenericArgument::Type(payload)) = generic_args.next() else {
        return None;
    };
    if generic_args.next().is_some() {
        return None;
    }
    match payload {
        Type::Tuple(tuple) => Some(tuple.elems.iter().cloned().collect()),
        other => Some(vec![other.clone()]),
    }
}

fn is_object_data_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "ObjectData";
        }
    }
    false
}

fn parse_property_attr(attr: &Attribute) -> PropertyAttr {
    let mut readonly = false;
    let mut notify = None;
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("readonly") {
            readonly = true;
        } else if meta.path.is_ident("notify") {
            notify = Some(meta.value()?.parse::<Ident>()?);
        }
        Ok(())
    });
    PropertyAttr { readonly, notify }
}

/// If `ty` is `Property<X>` (the reactive `QProperty<T>`-equivalent from `qtrs_core::property`),
/// returns `X`. A plain field type (including any other generic wrapper) returns `None`.
fn property_inner_type(ty: &Type) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Property" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let mut generic_args = args.args.iter();
    let Some(syn::GenericArgument::Type(inner)) = generic_args.next() else {
        return None;
    };
    if generic_args.next().is_some() {
        return None;
    }
    Some(inner.clone())
}

/// Strips whitespace so two independently-`quote!`d renderings of the same type compare equal.
fn normalize_type_string(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn generate_property_accessors(
    struct_name: &Ident,
    desc: &PropertyDesc,
    getter_ident: &Ident,
    setter_ident: &Ident,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let field_ident = &desc.field;
    let effective_type = &desc.effective_type;

    let read_expr = if desc.reactive {
        quote! { s.#field_ident.get() }
    } else {
        quote! { s.#field_ident.clone() }
    };

    let getter = quote! {
        #[allow(non_snake_case)]
        fn #getter_ident(obj: &dyn ::std::any::Any) -> ::qtrs_core::variant::Variant {
            if let Some(s) = obj.downcast_ref::<#struct_name>() {
                ::qtrs_core::variant::Variant::from(#read_expr)
            } else {
                ::qtrs_core::variant::Variant::Invalid
            }
        }
    };

    let setter = if !desc.readonly {
        // Only compare old vs. new (which requires `#effective_type: PartialEq`) when a
        // NOTIFY signal is actually configured; otherwise skip the comparison so properties
        // whose type doesn't implement `PartialEq` keep compiling as before.
        let write_body = match (&desc.notify_field, desc.reactive) {
            (Some(notify_field), true) => quote! {
                let __old = s.#field_ident.get();
                s.#field_ident.set(converted.clone());
                if converted != __old {
                    s.#notify_field.emit(&converted);
                }
            },
            (Some(notify_field), false) => quote! {
                let __old = s.#field_ident.clone();
                s.#field_ident = converted.clone();
                if converted != __old {
                    s.#notify_field.emit(&converted);
                }
            },
            (None, true) => quote! {
                s.#field_ident.set(converted);
            },
            (None, false) => quote! {
                s.#field_ident = converted;
            },
        };

        quote! {
            #[allow(non_snake_case)]
            fn #setter_ident(
                obj: &mut dyn ::std::any::Any,
                val: ::qtrs_core::variant::Variant,
            ) -> Result<(), ::qtrs_core::meta::InvokeError> {
                if let Some(s) = obj.downcast_mut::<#struct_name>() {
                    // Try to convert variant to property type
                    if let Some(converted) = ::qtrs_core::variant::convert_variant_to_type::<#effective_type>(&val) {
                        #write_body
                        Ok(())
                    } else {
                        Err(::qtrs_core::meta::InvokeError::TypeMismatch {
                            index: 0,
                            expected: stringify!(#effective_type),
                        })
                    }
                } else {
                    Err(::qtrs_core::meta::InvokeError::TargetBorrowFailed)
                }
            }
        }
    } else {
        quote! {
            #[allow(non_snake_case)]
            fn #setter_ident(
                _obj: &mut dyn ::std::any::Any,
                _val: ::qtrs_core::variant::Variant,
            ) -> Result<(), ::qtrs_core::meta::InvokeError> {
                Err(::qtrs_core::meta::InvokeError::ExecutionFailed("Property is read-only".into()))
            }
        }
    };

    (getter, setter)
}

/// Generates a `MethodInvoker` for a `#[signal]` field: converts each dynamically-passed
/// `Variant` argument to the signal's declared payload type(s) and calls `.emit(..)` with
/// them (as a tuple when there's more than one, matching `Signal<(A, B, ..)>`'s payload type).
fn generate_signal_invoker(
    struct_name: &Ident,
    field_ident: &Ident,
    payload_types: &[Type],
    invoker_ident: &Ident,
) -> proc_macro2::TokenStream {
    let arg_names: Vec<Ident> = (0..payload_types.len())
        .map(|i| Ident::new(&format!("__a{i}"), field_ident.span()))
        .collect();

    let conversions: Vec<_> = arg_names
        .iter()
        .zip(payload_types.iter())
        .enumerate()
        .map(|(i, (arg_name, ty))| {
            quote! {
                let #arg_name = match ::qtrs_core::variant::convert_variant_to_type::<#ty>(&args[#i]) {
                    Some(v) => v,
                    None => return Err(::qtrs_core::meta::InvokeError::TypeMismatch {
                        index: #i,
                        expected: stringify!(#ty),
                    }),
                };
            }
        })
        .collect();

    let payload_expr = match arg_names.as_slice() {
        [] => quote! { () },
        [single] => quote! { #single },
        many => quote! { ( #(#many),* ) },
    };

    quote! {
        #[allow(non_snake_case)]
        fn #invoker_ident(
            obj: &mut dyn ::std::any::Any,
            args: &[::qtrs_core::variant::Variant],
        ) -> Result<::qtrs_core::variant::Variant, ::qtrs_core::meta::InvokeError> {
            let Some(s) = obj.downcast_mut::<#struct_name>() else {
                return Err(::qtrs_core::meta::InvokeError::TargetBorrowFailed);
            };
            #(#conversions)*
            s.#field_ident.emit(&#payload_expr);
            Ok(::qtrs_core::variant::Variant::Invalid)
        }
    }
}
