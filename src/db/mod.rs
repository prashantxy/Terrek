// Source - https://stackoverflow.com/q/72763578
// Posted by cdaringe
// Retrieved 2026-02-02, License - CC BY-SA 4.0

use sqlx::sqlite::{SqlitePoolOptions};

SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite://{}", db_filename))
            .await
            .map_err(|err| format!("{}\nfile: {}", err.to_string(), db_filename))?;
