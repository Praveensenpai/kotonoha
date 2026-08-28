use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "dictionary_cache")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub expression: String,
    pub reading: Option<String>,
    pub definition: Option<String>,
    pub pitch_accent: Option<String>,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
