use crate::internals::*;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote_spanned;
use quote::{quote, ToTokens};
use syn::spanned::Spanned as _;
use syn::Meta::*;
use syn::MetaNameValue;
use syn::NestedMeta::*;

/// Parsed `#[rune(..)]` field attributes.
#[derive(Default)]
pub(crate) struct FieldAttrs {
    /// A field that is an identifier. Should use `Default::default` to be
    /// constructed and ignored during `ToTokens` and `Spanned`.
    pub(crate) id: Option<Span>,
    /// `#[rune(iter)]`
    pub(crate) iter: Option<Span>,
    /// `#[rune(skip)]`
    skip: Option<Span>,
    /// `#[rune(optional)]`
    pub(crate) optional: Option<Span>,
    /// `#[rune(meta)]`
    pub(crate) meta: Option<Span>,
    /// A single field marked with `#[rune(span)]`.
    pub(crate) span: Option<Span>,
    /// Custom parser `#[rune(parse_with = "..")]`.
    pub(crate) parse_with: Option<syn::Ident>,
}

impl FieldAttrs {
    /// Indicate if the field should be skipped.
    pub(crate) fn skip(&self) -> bool {
        self.skip.is_some() || self.id.is_some()
    }
}

/// The parsing implementations to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseKind {
    /// Generate default functions.
    Default,
    /// Only generate meta parse function.
    MetaOnly,
}

impl Default for ParseKind {
    fn default() -> Self {
        Self::Default
    }
}

/// Parsed ast derive attributes.
#[derive(Default)]
pub(crate) struct DeriveAttrs {
    /// The parse kind to build.
    pub(crate) parse: ParseKind,
}

pub(crate) struct Context {
    pub(crate) errors: Vec<syn::Error>,
    pub(crate) to_tokens: TokenStream,
    pub(crate) spanned: TokenStream,
    pub(crate) option_spanned: TokenStream,
    pub(crate) span: TokenStream,
    pub(crate) macro_context: TokenStream,
    pub(crate) token_stream: TokenStream,
    pub(crate) parser: TokenStream,
    pub(crate) parse: TokenStream,
    pub(crate) parse_error: TokenStream,
}

impl Context {
    /// Construct a new context.
    pub(crate) fn new() -> Self {
        Self::with_module(&quote!(crate))
    }

    /// Construct a new context.
    pub(crate) fn with_module<M>(module: M) -> Self
    where
        M: Copy + ToTokens,
    {
        Self {
            errors: Vec::new(),
            to_tokens: quote!(#module::ToTokens),
            spanned: quote!(#module::Spanned),
            option_spanned: quote!(#module::OptionSpanned),
            span: quote!(runestick::Span),
            macro_context: quote!(#module::MacroContext),
            token_stream: quote!(#module::TokenStream),
            parser: quote!(#module::Parser),
            parse: quote!(#module::Parse),
            parse_error: quote!(#module::ParseError),
        }
    }

    /// Get a field identifier.
    pub(crate) fn field_ident<'a>(&mut self, field: &'a syn::Field) -> Option<&'a syn::Ident> {
        match &field.ident {
            Some(ident) => Some(ident),
            None => {
                self.errors.push(syn::Error::new_spanned(
                    field,
                    "unnamed fields are not supported",
                ));
                None
            }
        }
    }

    /// Parse the toplevel component of the attribute, which must be `#[parse(..)]`.
    fn get_meta_items(
        &mut self,
        attr: &syn::Attribute,
        symbol: Symbol,
    ) -> Option<Vec<syn::NestedMeta>> {
        if attr.path != symbol {
            return Some(Vec::new());
        }

        match attr.parse_meta() {
            Ok(List(meta)) => Some(meta.nested.into_iter().collect()),
            Ok(other) => {
                self.errors.push(syn::Error::new_spanned(
                    other,
                    format!("expected #[{}(...)]", symbol),
                ));
                None
            }
            Err(error) => {
                self.errors.push(syn::Error::new(Span::call_site(), error));
                None
            }
        }
    }

    /// Parse field attributes.
    pub(crate) fn parse_derive_attributes(
        &mut self,
        input: &[syn::Attribute],
    ) -> Option<DeriveAttrs> {
        let mut attrs = DeriveAttrs::default();

        for attr in input {
            #[allow(clippy::never_loop)] // I guess this is on purpose?
            for meta in self.get_meta_items(attr, RUNE)? {
                match &meta {
                    // Parse `#[rune(id)]`
                    Meta(NameValue(MetaNameValue {
                        path,
                        lit: syn::Lit::Str(s),
                        ..
                    })) if *path == PARSE => {
                        let parse = match s.value().as_str() {
                            "meta_only" => ParseKind::MetaOnly,
                            other => {
                                self.errors.push(syn::Error::new_spanned(
                                    meta,
                                    format!(
                                        "unsupported `#[rune(parse = ..)]` argument `{}`",
                                        other
                                    ),
                                ));
                                return None;
                            }
                        };

                        attrs.parse = parse;
                    }
                    meta => {
                        self.errors
                            .push(syn::Error::new_spanned(meta, "unsupported attribute"));

                        return None;
                    }
                }
            }
        }

        Some(attrs)
    }

    /// Parse `#[rune(..)]` field attributes.
    pub(crate) fn parse_field_attributes(
        &mut self,
        input: &[syn::Attribute],
    ) -> Option<FieldAttrs> {
        let mut attrs = FieldAttrs::default();

        for attr in input {
            #[allow(clippy::never_loop)] // I guess this is on purpose?
            for meta in self.get_meta_items(attr, RUNE)? {
                match &meta {
                    // Parse `#[rune(id)]`
                    Meta(Path(word)) if *word == ID => {
                        attrs.id = Some(meta.span());
                    }
                    // Parse `#[rune(iter)]`.
                    Meta(Path(word)) if *word == ITER => {
                        attrs.iter = Some(meta.span());
                    }
                    // Parse `#[rune(skip)]`.
                    Meta(Path(word)) if *word == SKIP => {
                        attrs.skip = Some(meta.span());
                    }
                    // Parse `#[rune(optional)]`.
                    Meta(Path(word)) if *word == OPTIONAL => {
                        attrs.optional = Some(meta.span());
                    }
                    // Parse `#[rune(attributes)]`
                    Meta(Path(word)) if *word == META => {
                        attrs.meta = Some(meta.span());
                    }
                    // Parse `#[rune(span)]`
                    Meta(Path(word)) if *word == SPAN => {
                        attrs.span = Some(meta.span());
                    }
                    // Parse `#[rune(parse_with = "..")]`.
                    Meta(NameValue(MetaNameValue {
                        path,
                        lit: syn::Lit::Str(s),
                        ..
                    })) if *path == PARSE_WITH => {
                        if let Some(old) = attrs.parse_with {
                            let mut error = syn::Error::new_spanned(
                                path,
                                "#[rune(parse_with = \"..\")] can only be used once",
                            );

                            error.combine(syn::Error::new_spanned(old, "previously defined here"));
                            self.errors.push(error);
                            return None;
                        }

                        attrs.parse_with = Some(syn::Ident::new(&s.value(), s.span()));
                    }
                    meta => {
                        self.errors
                            .push(syn::Error::new_spanned(meta, "unsupported attribute"));

                        return None;
                    }
                }
            }
        }

        Some(attrs)
    }

    /// Build an inner spanned decoder from an iterator.
    pub(crate) fn build_spanned_iter<'a>(
        &mut self,
        back: bool,
        mut it: impl Iterator<Item = (Option<TokenStream>, &'a syn::Field)>,
    ) -> Option<(bool, Option<TokenStream>)> {
        let mut quote = None::<TokenStream>;

        loop {
            let (var, field) = match it.next() {
                Some((var, field)) => (var?, field),
                None => {
                    return Some((true, quote));
                }
            };

            let attrs = self.parse_field_attributes(&field.attrs)?;

            let spanned = &self.spanned;

            if attrs.skip() {
                continue;
            }

            if attrs.optional.is_some() {
                let option_spanned = &self.option_spanned;
                let next = quote_spanned! {
                    field.span() => #option_spanned::option_span(#var)
                };

                if quote.is_some() {
                    quote = Some(quote_spanned! {
                        field.span() => #quote.or_else(|| #next)
                    });
                } else {
                    quote = Some(next);
                }

                continue;
            }

            if attrs.iter.is_some() {
                let next = if back {
                    quote_spanned!(field.span() => next_back)
                } else {
                    quote_spanned!(field.span() => next)
                };

                let spanned = &self.spanned;
                let next = quote_spanned! {
                    field.span() => IntoIterator::into_iter(#var).#next().map(#spanned::span)
                };

                if quote.is_some() {
                    quote = Some(quote_spanned! {
                        field.span() => #quote.or_else(|| #next)
                    });
                } else {
                    quote = Some(next);
                }

                continue;
            }

            if quote.is_some() {
                quote = Some(quote_spanned! {
                    field.span() => #quote.unwrap_or_else(|| #spanned::span(#var))
                });
            } else {
                quote = Some(quote_spanned! {
                    field.span() => #spanned::span(#var)
                });
            }

            return Some((false, quote));
        }
    }

    /// Explicit span for fields.
    pub(crate) fn explicit_span(
        &mut self,
        named: &syn::FieldsNamed,
    ) -> Option<Option<TokenStream>> {
        let mut explicit_span = None;

        for field in &named.named {
            let attrs = self.parse_field_attributes(&field.attrs)?;

            if let Some(span) = attrs.span {
                if explicit_span.is_some() {
                    self.errors.push(syn::Error::new(
                        span,
                        "only one field can be marked `#[rune(span)]`",
                    ));
                    return None;
                }

                let ident = &field.ident;

                explicit_span = Some(quote_spanned! {
                    field.span() => self.#ident
                })
            }
        }

        Some(explicit_span)
    }
}
