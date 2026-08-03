use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;

        let db = Database { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS known_words (
                word TEXT PRIMARY KEY,
                added_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS ignored_words (
                word TEXT PRIMARY KEY,
                added_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS dictionary_cache (
                expression TEXT PRIMARY KEY,
                reading TEXT,
                definition TEXT,
                pitch_accent TEXT,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS mined_cards (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sentence TEXT NOT NULL,
                target_word TEXT NOT NULL,
                reading TEXT,
                definition TEXT,
                audio_path TEXT,
                image_path TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            DELETE FROM dictionary_cache WHERE definition LIKE '%[Noun] serif%' OR definition LIKE '%[Wikipedia definition] Serif%';
            ",
        )?;
        Ok(())
    }

    pub fn get_known_words(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("SELECT word FROM known_words")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut set = HashSet::new();
        for r in rows {
            set.insert(r?);
        }
        Ok(set)
    }

    pub fn add_known_words(&self, words: &[String]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut added = 0;
        {
            let mut stmt = tx.prepare("INSERT OR IGNORE INTO known_words (word) VALUES (?)")?;
            for w in words {
                if !w.trim().is_empty() {
                    added += stmt.execute(params![w.trim()])?;
                }
            }
        }
        tx.commit()?;
        Ok(added)
    }

    pub fn get_known_words_sorted(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT word FROM known_words ORDER BY word ASC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut words = Vec::new();
        for r in rows {
            words.push(r?);
        }
        Ok(words)
    }

    pub fn remove_known_words(&self, words: &[String]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut removed = 0;
        {
            let mut stmt = tx.prepare("DELETE FROM known_words WHERE word = ?")?;
            for w in words {
                removed += stmt.execute(params![w])?;
            }
        }
        tx.commit()?;
        Ok(removed)
    }

    pub fn get_ignored_words(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("SELECT word FROM ignored_words")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut set = HashSet::new();
        for r in rows {
            set.insert(r?);
        }
        Ok(set)
    }

    pub fn add_ignored_word(&self, word: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO ignored_words (word) VALUES (?)",
            params![word.trim()],
        )?;
        Ok(())
    }

    pub fn get_ignored_words_sorted(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT word FROM ignored_words ORDER BY word ASC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut words = Vec::new();
        for r in rows {
            words.push(r?);
        }
        Ok(words)
    }

    pub fn remove_ignored_words(&self, words: &[String]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut removed = 0;
        {
            let mut stmt = tx.prepare("DELETE FROM ignored_words WHERE word = ?")?;
            for w in words {
                removed += stmt.execute(params![w])?;
            }
        }
        tx.commit()?;
        Ok(removed)
    }

    pub fn get_cached_definition(&self, expression: &str) -> Result<Option<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT reading, definition, pitch_accent FROM dictionary_cache WHERE expression = ?",
        )?;
        let mut rows = stmt.query(params![expression])?;
        if let Some(row) = rows.next()? {
            let reading: String = row.get(0)?;
            let definition: String = row.get(1)?;
            let pitch: String = row.get(2)?;
            Ok(Some((reading, definition, pitch)))
        } else {
            Ok(None)
        }
    }

    pub fn cache_definition(&self, expression: &str, reading: &str, definition: &str, pitch: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO dictionary_cache (expression, reading, definition, pitch_accent) VALUES (?, ?, ?, ?)",
            params![expression, reading, definition, pitch],
        )?;
        Ok(())
    }

    pub fn save_mined_card(&self, sentence: &str, target_word: &str, reading: &str, definition: &str, audio_path: Option<&str>, image_path: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO mined_cards (sentence, target_word, reading, definition, audio_path, image_path) VALUES (?, ?, ?, ?, ?, ?)",
            params![sentence, target_word, reading, definition, audio_path, image_path],
        )?;
        Ok(())
    }
}
