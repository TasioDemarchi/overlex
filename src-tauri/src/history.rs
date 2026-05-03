// History module - SQLite-backed translation history with FTS5 search
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use rusqlite::{Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

/// History entry returned to frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryEntry {
    pub id: i64,
    pub original_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub engine: String,
    pub created_at: String,
}

/// Database struct for history operations
pub struct HistoryDb;

impl HistoryDb {
    /// Initialize the history database at the given path.
    /// Returns Err if already initialized or on error.
    pub fn init(path: &PathBuf) -> Result<(), String> {
        let conn = Connection::open(path)
            .map_err(|e| format!("Failed to open history DB: {}", e))?;

        // Create schema
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS translations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                original_text TEXT NOT NULL,
                translated_text TEXT NOT NULL,
                source_lang TEXT NOT NULL DEFAULT 'auto',
                target_lang TEXT NOT NULL,
                engine TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS translations_fts USING fts5(
                original_text, translated_text,
                content=translations,
                content_rowid=id
            );

            -- Triggers to keep FTS in sync
            CREATE TRIGGER IF NOT EXISTS translations_ai AFTER INSERT ON translations BEGIN
                INSERT INTO translations_fts(rowid, original_text, translated_text)
                VALUES (new.id, new.original_text, new.translated_text);
            END;

            CREATE TRIGGER IF NOT EXISTS translations_ad AFTER DELETE ON translations BEGIN
                INSERT INTO translations_fts(translations_fts, rowid, original_text, translated_text)
                VALUES ('delete', old.id, old.original_text, old.translated_text);
            END;
            "#
        ).map_err(|e| format!("Failed to create schema: {}", e))?;

        DB.set(Mutex::new(conn))
            .map_err(|_| "History DB already initialized".to_string())?;

        Ok(())
    }

    /// Get a locked reference to the DB connection
    fn get_conn() -> Result<&'static Mutex<Connection>, String> {
        DB.get().ok_or_else(|| "History DB not initialized".to_string())
    }

    /// Sanitize user input for FTS5 MATCH queries.
    /// Wraps the entire query in double quotes and escapes internal quotes
    /// to disable all FTS5 operators and treat input as a phrase search.
    pub fn sanitize_fts5_query(input: &str) -> String {
        // Escape internal double quotes by doubling them, then wrap in quotes
        let escaped = input.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    }

    /// Escape a field value for CSV output.
    /// If the field contains commas, double quotes, or newlines,
    /// wrap it in double quotes and escape internal quotes.
    pub fn escape_csv_field(field: &str) -> String {
        let needs_escape = field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r');
        if needs_escape {
            let escaped = field.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        } else {
            field.to_string()
        }
    }

    /// Insert a new history entry
    pub fn insert(entry: &HistoryEntry) -> Result<i64, String> {
        let conn = Self::get_conn()?.lock().unwrap();
        conn.execute(
            "INSERT INTO translations (original_text, translated_text, source_lang, target_lang, engine) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                &entry.original_text,
                &entry.translated_text,
                &entry.source_lang,
                &entry.target_lang,
                &entry.engine,
            ],
        ).map_err(|e| format!("Failed to insert history: {}", e))?;

        Ok(conn.last_insert_rowid())
    }

    /// Get all history entries with pagination (newest first)
    pub fn get_all(limit: u32, offset: u32) -> Result<Vec<HistoryEntry>, String> {
        let conn = Self::get_conn()?.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, original_text, translated_text, source_lang, target_lang, engine, created_at
             FROM translations ORDER BY id DESC LIMIT ?1 OFFSET ?2"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let entries = stmt.query_map(rusqlite::params![limit, offset], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                original_text: row.get(1)?,
                translated_text: row.get(2)?,
                source_lang: row.get(3)?,
                target_lang: row.get(4)?,
                engine: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).map_err(|e| format!("Failed to query history: {}", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| format!("Failed to collect history: {}", e))?;

        Ok(entries)
    }

    /// Search history using FTS5
    pub fn search(query: &str) -> Result<Vec<HistoryEntry>, String> {
        let conn = Self::get_conn()?.lock().unwrap();
        let sanitized = Self::sanitize_fts5_query(query);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.original_text, t.translated_text, t.source_lang, t.target_lang, t.engine, t.created_at
             FROM translations t
             JOIN translations_fts fts ON t.id = fts.rowid
             WHERE translations_fts MATCH ?1
             ORDER BY t.id DESC
             LIMIT 50"
        ).map_err(|e| format!("Failed to prepare search: {}", e))?;

        let entries = stmt.query_map(rusqlite::params![&sanitized], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                original_text: row.get(1)?,
                translated_text: row.get(2)?,
                source_lang: row.get(3)?,
                target_lang: row.get(4)?,
                engine: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).map_err(|e| format!("Failed to search history: {}", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| format!("Failed to collect search: {}", e))?;

        Ok(entries)
    }

    /// Export history as JSON or CSV string
    pub fn export(format: &str) -> Result<String, String> {
        let entries = Self::get_all(10000, 0)?;

        match format.to_lowercase().as_str() {
            "json" => {
                serde_json::to_string_pretty(&entries)
                    .map_err(|e| format!("Failed to serialize JSON: {}", e))
            }
            "csv" => {
                let mut csv = String::from("id,original_text,translated_text,source_lang,target_lang,engine,created_at\n");
                for entry in entries {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{}\n",
                        entry.id,
                        Self::escape_csv_field(&entry.original_text),
                        Self::escape_csv_field(&entry.translated_text),
                        Self::escape_csv_field(&entry.source_lang),
                        Self::escape_csv_field(&entry.target_lang),
                        Self::escape_csv_field(&entry.engine),
                        Self::escape_csv_field(&entry.created_at)
                    ));
                }
                Ok(csv)
            }
            _ => Err("Unsupported format. Use 'json' or 'csv'.".to_string()),
        }
    }

    /// Clear all history entries
    pub fn clear() -> Result<(), String> {
        let conn = Self::get_conn()?.lock().unwrap();
        // Delete from translations_fts first (trigger handles this, but being explicit)
        conn.execute("DELETE FROM translations", [])
            .map_err(|e| format!("Failed to clear history: {}", e))?;
        Ok(())
    }

    /// Delete a specific history entry by ID
    pub fn delete(id: i64) -> Result<(), String> {
        let conn = Self::get_conn()?.lock().unwrap();
        conn.execute("DELETE FROM translations WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| format!("Failed to delete history entry: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_history_crud() {
        // Use in-memory database for testing
        let conn = Connection::open_in_memory()
            .expect("Failed to open in-memory DB");

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS translations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                original_text TEXT NOT NULL,
                translated_text TEXT NOT NULL,
                source_lang TEXT NOT NULL DEFAULT 'auto',
                target_lang TEXT NOT NULL,
                engine TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS translations_fts USING fts5(
                original_text, translated_text,
                content=translations,
                content_rowid=id
            );

            CREATE TRIGGER IF NOT EXISTS translations_ai AFTER INSERT ON translations BEGIN
                INSERT INTO translations_fts(rowid, original_text, translated_text)
                VALUES (new.id, new.original_text, new.translated_text);
            END;

            CREATE TRIGGER IF NOT EXISTS translations_ad AFTER DELETE ON translations BEGIN
                INSERT INTO translations_fts(translations_fts, rowid, original_text, translated_text)
                VALUES ('delete', old.id, old.original_text, old.translated_text);
            END;
            "#
        ).expect("Failed to create schema");

        // Test insert
        let entry = HistoryEntry {
            id: 0,
            original_text: "Hello".to_string(),
            translated_text: "Hola".to_string(),
            source_lang: "en".to_string(),
            target_lang: "es".to_string(),
            engine: "test".to_string(),
            created_at: "2024-01-01 00:00:00".to_string(),
        };

        conn.execute(
            "INSERT INTO translations (original_text, translated_text, source_lang, target_lang, engine) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                &entry.original_text,
                &entry.translated_text,
                &entry.source_lang,
                &entry.target_lang,
                &entry.engine,
            ],
        ).expect("Failed to insert");

        // Test get
        let mut stmt = conn.prepare(
            "SELECT id, original_text, translated_text, source_lang, target_lang, engine, created_at FROM translations"
        ).expect("Failed to prepare");
        let mut rows = stmt.query([]).expect("Failed to query");
        let row = rows.next().expect("Expected a row").expect("Failed to get row");
        assert_eq!(row.get::<_, String>(1).expect("Failed to get original_text"), "Hello");

        // Test delete
        conn.execute("DELETE FROM translations WHERE id = 1", [])
            .expect("Failed to delete");
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM translations", [], |r| r.get(0))
            .expect("Failed to count");
        assert_eq!(count, 0);

        // Test clear
        conn.execute("INSERT INTO translations (original_text, translated_text, source_lang, target_lang, engine) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["a", "b", "en", "es", "test"]).expect("Failed to insert");
        conn.execute("DELETE FROM translations", []).expect("Failed to clear");
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM translations", [], |r| r.get(0))
            .expect("Failed to count");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_sanitize_fts5_query_simple() {
        // Simple text should be wrapped in quotes
        let result = HistoryDb::sanitize_fts5_query("hello world");
        assert_eq!(result, "\"hello world\"");
    }

    #[test]
    fn test_sanitize_fts5_query_with_quotes() {
        // Internal quotes should be doubled
        let result = HistoryDb::sanitize_fts5_query("say \"hello\"");
        assert_eq!(result, "\"say \"\"hello\"\"\"");
    }

    #[test]
    fn test_sanitize_fts5_query_with_operators() {
        // FTS5 operators like * should be disabled by wrapping in quotes
        let result = HistoryDb::sanitize_fts5_query("hello* world+");
        assert_eq!(result, "\"hello* world+\"");
    }

    #[test]
    fn test_escape_csv_field_simple() {
        // Simple text should pass through unchanged
        let result = HistoryDb::escape_csv_field("hello");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_escape_csv_field_with_comma() {
        // Fields with commas need quoting
        let result = HistoryDb::escape_csv_field("hello, world");
        assert_eq!(result, "\"hello, world\"");
    }

    #[test]
    fn test_escape_csv_field_with_quote() {
        // Fields with quotes need escaping
        let result = HistoryDb::escape_csv_field("say \"hi\"");
        assert_eq!(result, "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_escape_csv_field_with_newline() {
        // Fields with newlines need quoting
        let result = HistoryDb::escape_csv_field("hello\nworld");
        assert_eq!(result, "\"hello\nworld\"");
    }

    #[test]
    fn test_escape_csv_field_complex() {
        // Complex field with comma and quote
        let result = HistoryDb::escape_csv_field("Hello, \"World\"");
        assert_eq!(result, "\"Hello, \"\"World\"\"\"");
    }
}
