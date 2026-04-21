use super::is_han_char;
use tantivy_tokenizer_api::{Token, TokenFilter, TokenStream, Tokenizer};

/// Token filter that only keeps single Han character tokens.
///
/// All non-Han tokens (kana, hangul, latin, digits, symbols, etc.) are dropped.
#[derive(Clone, Copy, Debug, Default)]
pub struct HanOnlyFilter;

impl TokenFilter for HanOnlyFilter {
    type Tokenizer<T: Tokenizer> = HanOnlyWrapper<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> Self::Tokenizer<T> {
        HanOnlyWrapper { inner: tokenizer }
    }
}

#[derive(Clone)]
pub struct HanOnlyWrapper<T> {
    inner: T,
}

impl<T: Tokenizer> Tokenizer for HanOnlyWrapper<T> {
    type TokenStream<'a> = HanOnlyTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let mut stream = self.inner.token_stream(text);
        let mut output = Vec::new();
        let mut position: usize = 0;

        while stream.advance() {
            let token = stream.token();
            let first_char = token.text.chars().next();
            if let Some(c) = first_char
                && is_han_char(c)
                && token.text.chars().nth(1).is_none()
            {
                output.push(Token {
                    position,
                    ..token.clone()
                });
                position += 1;
            }
        }

        HanOnlyTokenStream {
            tokens: output,
            index: 0,
        }
    }
}

pub struct HanOnlyTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for HanOnlyTokenStream {
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
