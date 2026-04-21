//! Diacritic-only token filter for smartcase diacritic search.
//!
//! Keeps only tokens whose NFD form contains at least one foldable diacritic
//! mark, emitting them in their original (pre-fold) form. All other tokens
//! are dropped. This produces a sparse token stream used for the `diacritic`
//! index field.

use super::is_foldable_diacritic;
use tantivy_tokenizer_api::{Token, TokenFilter, TokenStream, Tokenizer};
use unicode_normalization::UnicodeNormalization;

/// Token filter that only keeps tokens containing foldable diacritics.
///
/// Scans each token's NFD form for characters matching [`is_foldable_diacritic`].
/// Tokens with at least one such character are emitted in their original
/// (NFKC-casefolded, pre-diacritic-fold) form; all others are dropped.
///
/// This is the dual of [`HanOnlyFilter`](crate::HanOnlyFilter): both produce
/// sparse output by keeping only tokens matching a specific criterion.
#[derive(Clone, Copy, Debug, Default)]
pub struct DiacriticOnlyFilter;

impl TokenFilter for DiacriticOnlyFilter {
    type Tokenizer<T: Tokenizer> = DiacriticOnlyWrapper<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> Self::Tokenizer<T> {
        DiacriticOnlyWrapper { inner: tokenizer }
    }
}

#[derive(Clone)]
pub struct DiacriticOnlyWrapper<T> {
    inner: T,
}

impl<T: Tokenizer> Tokenizer for DiacriticOnlyWrapper<T> {
    type TokenStream<'a> = DiacriticOnlyTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let mut stream = self.inner.token_stream(text);
        let mut output = Vec::new();
        let mut position: usize = 0;

        while stream.advance() {
            let token = stream.token();
            if token.text.nfd().any(is_foldable_diacritic) {
                output.push(Token {
                    position,
                    ..token.clone()
                });
                position += 1;
            }
        }

        DiacriticOnlyTokenStream {
            tokens: output,
            index: 0,
        }
    }
}

pub struct DiacriticOnlyTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for DiacriticOnlyTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}
