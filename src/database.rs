use std::{env, fs::create_dir_all};
use diesel::{Connection, SqliteConnection};
use directories::ProjectDirs;
use dotenvy::dotenv;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};

use crate::error::Result;

pub struct Database {
    connection: SqliteConnection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let connection = establish_connection()?;
        Ok(Self {
            connection
        })
    }
}


const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn establish_connection() -> Result<SqliteConnection> {
    let database_url = if cfg!(debug_assertions) {
        dotenv().ok();

        env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    } else {
        let dirs = ProjectDirs::from("", "", "ttt").expect("Failed to get base directory paths!");
        let data_folder = dirs.data_dir();

        create_dir_all(data_folder)
            .unwrap_or_else(|_| panic!("Failed to create data dir '{}'", data_folder.display()));

        data_folder
            .join("timetable.db")
            .to_str()
            .expect("Sorry non UTF-8 data directory names are not supported!")
            .to_owned()
    };

    let mut connection = SqliteConnection::establish(&database_url)?;

    use diesel_migrations::MigrationHarness;
    connection.run_pending_migrations(MIGRATIONS).unwrap();

    Ok(connection)
}
