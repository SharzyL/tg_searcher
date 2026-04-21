use super::{ScriptGroup, is_han_char, token_script_group};
use tantivy_tokenizer_api::{Token, TokenFilter, TokenStream, Tokenizer};

/// CJK bigram token filter with script group awareness.
///
/// Rules:
/// - Consecutive same-script-group CJK tokens produce overlapping bigrams.
///   - HanKana group: Han + Hiragana + Katakana
///   - Hangul group: Hangul syllables/jamo
/// - Cross-group adjacency does NOT produce bigrams.
/// - Isolated Han characters are dropped (covered by the unigram field).
/// - Isolated kana characters are kept as unigrams.
/// - Isolated hangul characters are kept as unigrams.
/// - Non-CJK tokens pass through unchanged.
#[derive(Clone, Copy, Debug, Default)]
pub struct CJKBigramFilter;

impl TokenFilter for CJKBigramFilter {
    type Tokenizer<T: Tokenizer> = CJKBigramWrapper<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> Self::Tokenizer<T> {
        CJKBigramWrapper { inner: tokenizer }
    }
}

#[derive(Clone)]
pub struct CJKBigramWrapper<T> {
    inner: T,
}

/// A token annotated with its script group.
struct AnnotatedToken {
    token: Token,
    group: ScriptGroup,
}

impl<T: Tokenizer> Tokenizer for CJKBigramWrapper<T> {
    type TokenStream<'a> = CJKBigramTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        // Collect and annotate all tokens.
        let mut annotated = Vec::new();
        let mut stream = self.inner.token_stream(text);
        while stream.advance() {
            let token = stream.token().clone();
            let group = token_script_group(&token.text);
            annotated.push(AnnotatedToken { token, group });
        }

        let mut output = Vec::new();
        let mut position: usize = 0;
        let len = annotated.len();
        let mut i = 0;

        while i < len {
            let group = annotated[i].group;

            if group == ScriptGroup::NonCjk {
                // Pass through non-CJK tokens unchanged.
                output.push(Token {
                    position,
                    ..annotated[i].token.clone()
                });
                position += 1;
                i += 1;
                continue;
            }

            // Collect a run of same-group CJK tokens.
            let run_start = i;
            i += 1;
            while i < len && annotated[i].group == group {
                i += 1;
            }
            let run = &annotated[run_start..i];

            if run.len() == 1 {
                // Isolated CJK token.
                if is_isolated_han(&run[0]) {
                    // Isolated Han: drop (unigram field covers it).
                    continue;
                }
                // Isolated kana or hangul: keep as unigram.
                output.push(Token {
                    position,
                    ..run[0].token.clone()
                });
                position += 1;
                continue;
            }

            // Emit overlapping bigrams for the run.
            for pair in run.windows(2) {
                output.push(Token {
                    offset_from: pair[0].token.offset_from,
                    offset_to: pair[1].token.offset_to,
                    position,
                    text: format!("{}{}", pair[0].token.text, pair[1].token.text),
                    position_length: 1,
                });
                position += 1;
            }
        }

        CJKBigramTokenStream {
            tokens: output,
            index: 0,
        }
    }
}

/// Returns true if the token is an isolated Han character (single char, is Han).
fn is_isolated_han(at: &AnnotatedToken) -> bool {
    let mut chars = at.token.text.chars();
    match chars.next() {
        Some(c) if is_han_char(c) => chars.next().is_none(),
        _ => false,
    }
}

pub struct CJKBigramTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for CJKBigramTokenStream {
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
