#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;
mod m20220101_000001_users;

mod m20260927_123453_posts;
mod m20261003_213820_chain_snapshots;
mod m20261003_214239_chain_blocks;
mod m20261008_152453_chain_alerts;
mod m20261010_153838_remove_users_table;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_users::Migration),
            Box::new(m20260927_123453_posts::Migration),
            Box::new(m20261003_213820_chain_snapshots::Migration),
            Box::new(m20261003_214239_chain_blocks::Migration),
            Box::new(m20261008_152453_chain_alerts::Migration),
            Box::new(m20261010_153838_remove_users_table::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
