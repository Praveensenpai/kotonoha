use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "ai_analysis_cache")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub cache_key: String,
    pub english_natural: Option<String>,
    pub english_literal: Option<String>,
    pub kannada_natural: Option<String>,
    pub kannada_literal: Option<String>,
    pub parsing_warning: Option<String>,
    pub recommended_candidate_index: Option<i64>,
    pub recommended_sense_index: Option<i64>,
    pub custom_definition_suggestion: Option<String>,
    pub explanation: Option<String>,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
