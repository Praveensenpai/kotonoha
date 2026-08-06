use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::Path;

pub struct Database {
    conn: Connection,
}

pub struct MinedCard {
    pub id: i64,
    pub sentence: String,
    pub target_word: String,
    pub reading: String,
    pub pitch_accent: String,
    pub definition: String,
    pub audio_path: Option<String>,
    pub image_path: Option<String>,
    pub english_natural: Option<String>,
    pub english_literal: Option<String>,
    pub kannada_natural: Option<String>,
    pub kannada_literal: Option<String>,
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
        let columns = self
            .conn
            .prepare("PRAGMA table_info(mined_cards)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if !columns.iter().any(|column| column == "anki_note_id") {
            self.conn.execute("ALTER TABLE mined_cards ADD COLUMN anki_note_id INTEGER", [])?;
        }
        if !columns.iter().any(|column| column == "pitch_accent") {
            self.conn.execute("ALTER TABLE mined_cards ADD COLUMN pitch_accent TEXT", [])?;
        }
        if !columns.iter().any(|column| column == "english_natural") {
            self.conn.execute("ALTER TABLE mined_cards ADD COLUMN english_natural TEXT", [])?;
        }
        if !columns.iter().any(|column| column == "english_literal") {
            self.conn.execute("ALTER TABLE mined_cards ADD COLUMN english_literal TEXT", [])?;
        }
        if !columns.iter().any(|column| column == "kannada_natural") {
            self.conn.execute("ALTER TABLE mined_cards ADD COLUMN kannada_natural TEXT", [])?;
        }
        if !columns.iter().any(|column| column == "kannada_literal") {
            self.conn.execute("ALTER TABLE mined_cards ADD COLUMN kannada_literal TEXT", [])?;
        }

        let kw_columns = self
            .conn
            .prepare("PRAGMA table_info(known_words)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if !kw_columns.iter().any(|column| column == "source") {
            self.conn.execute("ALTER TABLE known_words ADD COLUMN source TEXT DEFAULT 'known'", [])?;
        }
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

    pub fn get_known_words_by_source(&self, source: &str) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("SELECT word FROM known_words WHERE source = ?")?;
        let rows = stmt.query_map(params![source], |row| row.get(0))?;
        let mut set = HashSet::new();
        for r in rows {
            set.insert(r?);
        }
        Ok(set)
    }

    pub fn add_known_words(&self, words: &[String]) -> Result<usize> {
        self.add_known_words_with_source(words, "known")
    }

    pub fn add_known_words_with_source(&self, words: &[String], source: &str) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut added = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO known_words (word, source) VALUES (?, ?) ON CONFLICT(word) DO UPDATE SET source = excluded.source",
            )?;
            for w in words {
                if !w.trim().is_empty() {
                    added += stmt.execute(params![w.trim(), source])?;
                }
            }
        }
        tx.commit()?;
        Ok(added)
    }

    #[allow(dead_code)]
    pub fn get_known_words_sorted(&self) -> Result<Vec<String>> {
        self.get_known_words_sorted_by_source("known")
    }

    pub fn get_known_words_sorted_by_source(&self, source: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT word FROM known_words WHERE source = ? ORDER BY word ASC")?;
        let rows = stmt.query_map(params![source], |row| row.get(0))?;
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
            // A previous version cached this sentinel when Jisho returned no
            // usable result. Treat it as a miss so the dictionary is retried.
            if definition.trim() == "1. [def] vocabulary word"
                || definition.trim() == "No dictionary definition found"
            {
                Ok(None)
            } else {
                Ok(Some((reading, definition, pitch)))
            }
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

    pub fn clear_dictionary_cache(&self) -> Result<usize> {
        let count = self.conn.execute("DELETE FROM dictionary_cache", [])?;
        Ok(count)
    }
}

pub struct SaveMinedCardParams<'a> {
    pub sentence: &'a str,
    pub target_word: &'a str,
    pub reading: &'a str,
    pub pitch_accent: &'a str,
    pub definition: &'a str,
    pub audio_path: Option<&'a str>,
    pub image_path: Option<&'a str>,
    pub english_natural: Option<&'a str>,
    pub english_literal: Option<&'a str>,
    pub kannada_natural: Option<&'a str>,
    pub kannada_literal: Option<&'a str>,
}

impl Database {
    pub fn save_mined_card(&self, p: SaveMinedCardParams<'_>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO mined_cards (sentence, target_word, reading, pitch_accent, definition, audio_path, image_path, english_natural, english_literal, kannada_natural, kannada_literal) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![p.sentence, p.target_word, p.reading, p.pitch_accent, p.definition, p.audio_path, p.image_path, p.english_natural, p.english_literal, p.kannada_natural, p.kannada_literal],
        )?;
        Ok(())
    }

    pub fn get_unsynced_mined_cards(&self) -> Result<Vec<MinedCard>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, sentence, target_word, reading, pitch_accent, definition, audio_path, image_path, english_natural, english_literal, kannada_natural, kannada_literal
             FROM mined_cards WHERE anki_note_id IS NULL ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MinedCard {
                id: row.get(0)?,
                sentence: row.get(1)?,
                target_word: row.get(2)?,
                reading: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                pitch_accent: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                definition: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                audio_path: row.get(6)?,
                image_path: row.get(7)?,
                english_natural: row.get(8)?,
                english_literal: row.get(9)?,
                kannada_natural: row.get(10)?,
                kannada_literal: row.get(11)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_cards_missing_translations(&self) -> Result<Vec<MinedCard>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, sentence, target_word, reading, pitch_accent, definition, audio_path, image_path, english_natural, english_literal, kannada_natural, kannada_literal
             FROM mined_cards WHERE english_natural IS NULL OR kannada_natural IS NULL ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MinedCard {
                id: row.get(0)?,
                sentence: row.get(1)?,
                target_word: row.get(2)?,
                reading: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                pitch_accent: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                definition: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                audio_path: row.get(6)?,
                image_path: row.get(7)?,
                english_natural: row.get(8)?,
                english_literal: row.get(9)?,
                kannada_natural: row.get(10)?,
                kannada_literal: row.get(11)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_all_mined_cards(&self) -> Result<Vec<(MinedCard, Option<i64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, sentence, target_word, reading, pitch_accent, definition, audio_path, image_path, english_natural, english_literal, kannada_natural, kannada_literal, anki_note_id
             FROM mined_cards ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            let card = MinedCard {
                id: row.get(0)?,
                sentence: row.get(1)?,
                target_word: row.get(2)?,
                reading: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                pitch_accent: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                definition: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                audio_path: row.get(6)?,
                image_path: row.get(7)?,
                english_natural: row.get(8)?,
                english_literal: row.get(9)?,
                kannada_natural: row.get(10)?,
                kannada_literal: row.get(11)?,
            };
            let note_id: Option<i64> = row.get(12)?;
            Ok((card, note_id))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_card_translations(
        &self,
        card_id: i64,
        eng_nat: Option<&str>,
        eng_lit: Option<&str>,
        kan_nat: Option<&str>,
        kan_lit: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE mined_cards SET english_natural = ?, english_literal = ?, kannada_natural = ?, kannada_literal = ? WHERE id = ?",
            params![eng_nat, eng_lit, kan_nat, kan_lit, card_id],
        )?;
        Ok(())
    }

    pub fn mark_mined_card_synced(&self, card_id: i64, anki_note_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE mined_cards SET anki_note_id = ? WHERE id = ?",
            params![anki_note_id, card_id],
        )?;
        Ok(())
    }
}
