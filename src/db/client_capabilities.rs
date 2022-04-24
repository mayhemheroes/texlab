use std::sync::Arc;

use lsp_types::{ClientCapabilities, MarkupKind};

#[salsa::query_group(ClientCapabilitiesDatabaseStorage)]
pub trait ClientCapabilitiesDatabase: salsa::Database {
    #[salsa::input]
    fn client_capabilities(&self) -> Arc<ClientCapabilities>;

    fn has_definition_link_support(&self) -> bool;

    fn has_hierarchical_document_symbol_support(&self) -> bool;

    fn has_work_done_progress_support(&self) -> bool;

    fn has_snippet_support(&self) -> bool;

    fn has_completion_markdown_support(&self) -> bool;

    fn has_hover_markdown_support(&self) -> bool;

    fn has_pull_configuration_support(&self) -> bool;

    fn has_push_configuration_support(&self) -> bool;

    fn has_file_watching_support(&self) -> bool;
}

fn has_definition_link_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .text_document
        .as_ref()
        .and_then(|cap| cap.definition.as_ref())
        .and_then(|cap| cap.link_support)
        == Some(true)
}

fn has_hierarchical_document_symbol_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .text_document
        .as_ref()
        .and_then(|cap| cap.document_symbol.as_ref())
        .and_then(|cap| cap.hierarchical_document_symbol_support)
        == Some(true)
}

fn has_work_done_progress_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .window
        .as_ref()
        .and_then(|cap| cap.work_done_progress)
        == Some(true)
}

fn has_snippet_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .text_document
        .as_ref()
        .and_then(|cap| cap.completion.as_ref())
        .and_then(|cap| cap.completion_item.as_ref())
        .and_then(|cap| cap.snippet_support)
        == Some(true)
}

fn has_completion_markdown_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .text_document
        .as_ref()
        .and_then(|cap| cap.completion.as_ref())
        .and_then(|cap| cap.completion_item.as_ref())
        .and_then(|cap| cap.documentation_format.as_ref())
        .map_or(false, |formats| formats.contains(&MarkupKind::Markdown))
}

fn has_hover_markdown_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .text_document
        .as_ref()
        .and_then(|cap| cap.hover.as_ref())
        .and_then(|cap| cap.content_format.as_ref())
        .map_or(false, |formats| formats.contains(&MarkupKind::Markdown))
}

fn has_pull_configuration_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .workspace
        .as_ref()
        .and_then(|cap| cap.configuration)
        == Some(true)
}

fn has_push_configuration_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .workspace
        .as_ref()
        .and_then(|cap| cap.did_change_configuration)
        .and_then(|cap| cap.dynamic_registration)
        == Some(true)
}

fn has_file_watching_support(db: &dyn ClientCapabilitiesDatabase) -> bool {
    db.client_capabilities()
        .workspace
        .as_ref()
        .and_then(|cap| cap.did_change_watched_files)
        .and_then(|cap| cap.dynamic_registration)
        == Some(true)
}
