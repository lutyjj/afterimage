//! Builds the schema the query macros check every query against: a SQLite
//! database made from the server's migrations, as the server itself migrates
//! one, named to the macros through `DATABASE_URL`. The migrations are the only
//! copy of the schema, so no query metadata is kept beside them.

use std::path::{Path, PathBuf};

use sqlx::Connection;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};

fn main() {
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    println!("cargo::rerun-if-changed={}", migrations.display());

    let schema =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("schema.db");
    match std::fs::remove_file(&schema) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            panic!("removing the previous {}: {error}", schema.display())
        }
        _ => {}
    }
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime for the migrations")
        .block_on(async {
            let options = SqliteConnectOptions::new()
                .filename(&schema)
                .create_if_missing(true);
            let mut connection = SqliteConnection::connect_with(&options).await?;
            Migrator::new(migrations.as_path())
                .await?
                .run(&mut connection)
                .await?;
            connection.close().await?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .unwrap_or_else(|error| {
            panic!("building the schema from {}: {error}", migrations.display())
        });

    println!(
        "cargo::rustc-env=DATABASE_URL=sqlite://{}",
        schema.display()
    );
}
