use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "mined_cards")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub sentence: String,
    pub target_word: String,
    pub reading: Option<String>,
    pub pitch_accent: Option<String>,
    pub definition: Option<String>,
    pub audio_path: Option<String>,
    pub image_path: Option<String>,
    pub english_natural: Option<String>,
    pub english_literal: Option<String>,
    pub kannada_natural: Option<String>,
    pub kannada_literal: Option<String>,
    pub anki_note_id: Option<i64>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
