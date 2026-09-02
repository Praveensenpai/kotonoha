use anyhow::{Context, Result};
use sea_orm::{
    ConnectionTrait, Database as SeaDatabase, DatabaseConnection,
};
use std::path::Path;

pub mod bundles;
pub mod cache;
pub mod cards;
pub mod entities;
pub mod words;

pub use cache::*;
pub use cards::*;
pub use entities::*;

#[derive(Debug, Clone)]
pub struct Database {
    conn: DatabaseConnection,
}

impl Database {
    pub async fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let abs_path = if db_path.is_absolute() {
            db_path.to_path_buf()
        } else {
            std::env::current_dir()?.join(db_path)
        };

        let db_url = format!("sqlite://{}?mode=rwc", abs_path.to_string_lossy());
        let conn = SeaDatabase::connect(&db_url)
            .await
            .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;

        let db = Database { conn };
        db.init_schema().await?;
        Ok(db)
    }

    async fn init_schema(&self) -> Result<()> {
        let builder = self.conn.get_database_backend();
        let schema = sea_orm::Schema::new(builder);

        let table_stmts = [
            schema.create_table_from_entity(KnownWords).if_not_exists().take(),
            schema.create_table_from_entity(IgnoredWords).if_not_exists().take(),
            schema.create_table_from_entity(DictionaryCache).if_not_exists().take(),
            schema.create_table_from_entity(AllCandidatesCache).if_not_exists().take(),
            schema.create_table_from_entity(MinedCards).if_not_exists().take(),
            schema.create_table_from_entity(AiAnalysisCache).if_not_exists().take(),
            schema.create_table_from_entity(OfflineTerms).if_not_exists().take(),
            schema.create_table_from_entity(BundledMedia).if_not_exists().take(),
        ];

        for stmt in table_stmts {
            self.conn.execute(builder.build(&stmt)).await?;
        }

        self.conn.execute_unprepared(
            "
            CREATE INDEX IF NOT EXISTS idx_offline_terms_expr ON offline_terms(expression);
            CREATE INDEX IF NOT EXISTS idx_offline_terms_reading ON offline_terms(reading);
            CREATE INDEX IF NOT EXISTS idx_bundled_media_fps ON bundled_media(video_fingerprint, subtitle_fingerprint);
            DELETE FROM dictionary_cache WHERE definition LIKE '%[Noun] serif%' OR definition LIKE '%[Wikipedia definition] Serif%';
            ",
        ).await?;

        // Run migrations for any existing databases that might miss new columns
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN anki_note_id INTEGER").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN pitch_accent TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN english_natural TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN english_literal TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN kannada_natural TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN kannada_literal TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE ai_analysis_cache ADD COLUMN recommended_candidate_index INTEGER").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE ai_analysis_cache ADD COLUMN recommended_sense_index INTEGER").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE ai_analysis_cache ADD COLUMN custom_definition_suggestion TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE ai_analysis_cache ADD COLUMN explanation TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE known_words ADD COLUMN source TEXT DEFAULT 'known'").await;

        Ok(())
    }
}
