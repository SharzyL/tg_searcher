#![doc = include_str!("../README.md")]
//!
//! # Module Overview
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`filter`] | Token filters: [`CJKBigramFilter`], [`HanOnlyFilter`], [`DiacriticFoldingFilter`], [`DiacriticOnlyFilter`], [`SemiticNormalizationFilter`], and script classification utilities |
//! | [`normalizer`] | [`NormalizedText`]: NFKC Casefold normalization with byte offset mapping |
//! | [`cjk`] | CJK character detection ([`is_cjk_char`]) and unigram expansion |
//! | [`word_break`] | ICU word break iterator wrapper |
//! | [`search`] | *(feature `tantivy-search`)* [`ICUSearchConfig`](search::ICUSearchConfig): three-field schema, smartcase query routing, snippet generation |
//! | `demo` | *(feature `demo`)* Test harness with query test cases |

pub mod cjk;
#[cfg(feature = "demo")]
pub mod demo;
pub mod filter;
pub mod normalizer;
#[cfg(feature = "tantivy-search")]
pub mod search;
mod tokenizer;
pub mod word_break;

pub use cjk::is_cjk_char;
pub use filter::{
    CJKBigramFilter, DiacriticFoldingFilter, DiacriticOnlyFilter, HanOnlyFilter, ScriptGroup,
    SemiticNormalizationFilter, find_isolated_han_tokens, has_foldable_diacritic,
    is_foldable_diacritic, is_han_char, token_script_group,
};

pub use normalizer::NormalizedText;
pub use tokenizer::NormalizingICUTokenizer;
