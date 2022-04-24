use std::sync::Arc;

use crate::distro::{DistributionKind, Resolver};

#[salsa::query_group(DistroDatabaseStorage)]
pub trait DistroDatabase: salsa::Database {
    #[salsa::input]
    fn distro_kind(&self) -> DistributionKind;

    #[salsa::input]
    fn distro_resolver(&self) -> Arc<Resolver>;
}
