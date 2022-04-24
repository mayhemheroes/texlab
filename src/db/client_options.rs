use std::{path::PathBuf, sync::Arc};

use crate::Options;

use super::LocationDatabase;

#[salsa::query_group(ClientOptionsDatabaseStorage)]
pub trait ClientOptionsDatabase: salsa::Database + LocationDatabase {
    #[salsa::input]
    fn client_options(&self) -> Arc<Options>;

    fn root_directory(&self) -> Option<Arc<PathBuf>>;

    fn aux_directory(&self) -> Option<Arc<PathBuf>>;
}

fn root_directory(db: &dyn ClientOptionsDatabase) -> Option<Arc<PathBuf>> {
    db.client_options()
        .root_directory
        .as_deref()
        .map(|dir| Arc::new(db.current_directory().join(dir)))
}

fn aux_directory(db: &dyn ClientOptionsDatabase) -> Option<Arc<PathBuf>> {
    db.client_options()
        .aux_directory
        .as_deref()
        .map(|dir| Arc::new(db.current_directory().join(dir)))
}
