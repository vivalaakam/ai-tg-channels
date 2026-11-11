use bin_common::{init_tracing, load_env, require};
use migration::{Migrator, MigratorTrait};
use sea_orm::Database;

#[tokio::main]
async fn main() {
    load_env();
    init_tracing();

    let db_url = require("DATABASE_URL");
    let db = match Database::connect(&db_url).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!("Failed to connect to database: {e}");
            std::process::exit(1);
        }
    };

    match Migrator::up(&db, None).await {
        Ok(()) => tracing::info!("Migrations applied"),
        Err(e) => {
            tracing::error!("Migration failed: {e}");
            std::process::exit(1);
        }
    }
}
