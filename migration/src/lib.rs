pub use sea_orm_migration::MigrationTrait;

mod m20240619000000_create_channels;
mod m20240619000001_create_messages;
mod m20240619000002_add_is_checked_to_messages;
mod m20240619000003_create_feed;
mod m20240619000004_create_feed_messages;
mod m20240619000005_add_post_type_to_feed;
mod m20260624000006_create_app_config;
mod m20260624000007_create_prompts;
mod m20260624000008_seed_prompts;

pub use sea_orm_migration::MigratorTrait;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240619000000_create_channels::Migration),
            Box::new(m20240619000001_create_messages::Migration),
            Box::new(m20240619000002_add_is_checked_to_messages::Migration),
            Box::new(m20240619000003_create_feed::Migration),
            Box::new(m20240619000004_create_feed_messages::Migration),
            Box::new(m20240619000005_add_post_type_to_feed::Migration),
            Box::new(m20260624000006_create_app_config::Migration),
            Box::new(m20260624000007_create_prompts::Migration),
            Box::new(m20260624000008_seed_prompts::Migration),
        ]
    }
}
