use std::sync::Arc;

use lsp_types::ClientInfo;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum KnownClient {
    Code,
    Unknown,
}

impl Default for KnownClient {
    fn default() -> Self {
        Self::Unknown
    }
}

#[salsa::query_group(ClientInfoDatabaseStorage)]
pub trait ClientInfoDatabase: salsa::Database {
    #[salsa::input]
    fn client_info(&self) -> Option<Arc<ClientInfo>>;

    fn client_kind(&self) -> KnownClient;
}

fn client_kind(db: &dyn ClientInfoDatabase) -> KnownClient {
    db.client_info()
        .as_deref()
        .map(|info| match info.name.as_str() {
            "Visual Studio Code" => KnownClient::Code,
            _ => KnownClient::Unknown,
        })
        .unwrap_or_default()
}
