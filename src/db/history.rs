use rusqlite::{Connection, Result};

pub fn get_history(limit: i64) -> Result<Vec<(String, String, i64)>> {
    let conn = Connection::open("terrek.db")?;

    let mut stmt = conn.prepare(
        "SELECT command, output, timestamp
         FROM commands
         ORDER BY timestamp DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map([limit], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
        ))
    })?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn search_history(keyword: &str) -> Result<Vec<(String, String, i64)>> {
    let conn = Connection::open("terrek.db")?;

    let mut stmt = conn.prepare(
        "SELECT command, output, timestamp
         FROM commands
         WHERE command LIKE ?1
         ORDER BY timestamp DESC",
    )?;

    let pattern = format!("%{}%", keyword);

    let rows = stmt.query_map([pattern], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
        ))
    })?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}
