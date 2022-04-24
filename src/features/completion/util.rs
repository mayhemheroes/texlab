use lsp_types::{CompletionItemKind, CompletionParams, Documentation, MarkupContent, MarkupKind};
use smol_str::SmolStr;

use crate::{db::ClientCapabilitiesDatabase, features::FeatureRequest};

pub fn component_detail(file_names: &[SmolStr]) -> String {
    if file_names.is_empty() {
        "built-in".to_owned()
    } else {
        file_names.join(", ")
    }
}

pub fn image_documentation(
    request: &FeatureRequest<CompletionParams>,
    name: &str,
    image: &str,
) -> Option<Documentation> {
    if supports_images(request) {
        Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!(
                "![{}](data:image/png;base64,{}|width=48,height=48)",
                name, image
            ),
        }))
    } else {
        None
    }
}

fn supports_images(request: &FeatureRequest<CompletionParams>) -> bool {
    request.db.has_completion_markdown_support()
}

pub fn adjust_kind(
    request: &FeatureRequest<CompletionParams>,
    kind: CompletionItemKind,
) -> CompletionItemKind {
    if request
        .db
        .client_capabilities()
        .text_document
        .as_ref()
        .and_then(|cap| cap.completion.as_ref())
        .and_then(|cap| cap.completion_item_kind.as_ref())
        .and_then(|cap| cap.value_set.as_ref())
        .map_or(false, |value_set| value_set.contains(&kind))
    {
        kind
    } else {
        CompletionItemKind::TEXT
    }
}
