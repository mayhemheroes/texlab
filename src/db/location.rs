use std::{path::PathBuf, sync::Arc};

#[salsa::query_group(LocationDatabaseStorage)]
pub trait LocationDatabase: salsa::Database {
    #[salsa::input]
    fn current_directory(&self) -> Arc<PathBuf>;
}
