use tantivy::schema::*;

#[derive(Clone)]
pub struct SchemaFields {
    pub id: Field,
    pub body: Field,
    pub body_bigram: Field,
    pub body_unigram: Field,
}

pub fn build_schema() -> (Schema, SchemaFields) {
    let mut builder = Schema::builder();

    let id = builder.add_text_field("id", STRING | STORED);
    let body = builder.add_text_field("body", STORED);

    let bigram_indexing = TextFieldIndexing::default()
        .set_tokenizer("cjk_bigram")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let body_bigram = builder.add_text_field(
        "body_bigram",
        TextOptions::default().set_indexing_options(bigram_indexing),
    );

    let unigram_indexing = TextFieldIndexing::default()
        .set_tokenizer("cjk_unigram")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let body_unigram = builder.add_text_field(
        "body_unigram",
        TextOptions::default().set_indexing_options(unigram_indexing),
    );

    (
        builder.build(),
        SchemaFields {
            id,
            body,
            body_bigram,
            body_unigram,
        },
    )
}
