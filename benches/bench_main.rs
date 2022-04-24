use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lsp_types::{
    CompletionParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
};
use texlab::{
    db::{DocumentData, DocumentDatabase, RootDatabase},
    features::FeatureRequest,
    syntax::latex,
    DocumentLanguage,
};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("LaTeX/Parser", |b| {
        b.iter(|| latex::parse(black_box(include_str!("../texlab.tex"))));
    });

    c.bench_function("LaTeX/Completion/Command", |b| {
        let uri = Url::parse("http://example.com/texlab.tex").unwrap();
        let text = Arc::new(include_str!("../texlab.tex").to_string());
        let mut db = RootDatabase::default();
        let document = db.intern_document(DocumentData::from(uri.clone()));
        db.upsert_document(document, text, DocumentLanguage::Latex);

        b.iter(|| {
            texlab::features::complete(FeatureRequest {
                params: CompletionParams {
                    context: None,
                    partial_result_params: Default::default(),
                    work_done_progress_params: Default::default(),
                    text_document_position: TextDocumentPositionParams::new(
                        TextDocumentIdentifier::new(uri.clone()),
                        Position::new(0, 1),
                    ),
                },
                db: &db,
                document,
            })
        });
    });
}

criterion_group!(benches, criterion_benchmark);

criterion_main!(benches);
