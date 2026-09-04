use anyhow::{Context, Result};
use sea_orm::{sea_query::OnConflict, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use std::path::{Path, PathBuf};

use super::entities::*;
use super::Database;

impl Database {
    pub async fn record_bundle(
        &self,
        bundle_path: &Path,
        source_video: &str,
        source_subtitle: &str,
        video_fingerprint: &str,
        subtitle_fingerprint: &str,
    ) -> Result<()> {
        let active = bundled_media::ActiveModel {
            bundle_path: Set(bundle_path.to_string_lossy().to_string()),
            source_video: Set(source_video.to_string()),
            source_subtitle: Set(source_subtitle.to_string()),
            video_fingerprint: Set(video_fingerprint.to_string()),
            subtitle_fingerprint: Set(subtitle_fingerprint.to_string()),
            created_at: Set(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        };

        BundledMedia::insert(active)
            .on_conflict(
                OnConflict::column(bundled_media::Column::BundlePath)
                    .update_columns([
                        bundled_media::Column::SourceVideo,
                        bundled_media::Column::SourceSubtitle,
                        bundled_media::Column::VideoFingerprint,
                        bundled_media::Column::SubtitleFingerprint,
                        bundled_media::Column::CreatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.conn)
            .await?;

        Ok(())
    }

    pub async fn find_existing_bundle(
        &self,
        video_fingerprint: &str,
        subtitle_fingerprint: &str,
    ) -> Result<Option<PathBuf>> {
        let match_record = BundledMedia::find()
            .filter(bundled_media::Column::VideoFingerprint.eq(video_fingerprint))
            .filter(bundled_media::Column::SubtitleFingerprint.eq(subtitle_fingerprint))
            .order_by_desc(bundled_media::Column::Id)
            .one(&self.conn)
            .await?;

        if let Some(record) = match_record {
            let path = PathBuf::from(&record.bundle_path);
            if path.exists() {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    pub async fn get_all_bundled_media(&self) -> Result<Vec<bundled_media::Model>> {
        BundledMedia::find()
            .order_by_desc(bundled_media::Column::Id)
            .all(&self.conn)
            .await
            .context("Failed to load bundled media list")
    }

    pub async fn delete_bundled_media_by_id(&self, id: i32) -> Result<()> {
        BundledMedia::delete_by_id(id)
            .exec(&self.conn)
            .await
            .context("Failed to delete bundled media by id")?;
        Ok(())
    }

    pub async fn delete_bundled_media_by_path(&self, bundle_path: &str) -> Result<()> {
        BundledMedia::delete_many()
            .filter(bundled_media::Column::BundlePath.eq(bundle_path))
            .exec(&self.conn)
            .await
            .context("Failed to delete bundled media by path")?;
        Ok(())
    }

    pub async fn prune_missing_bundles(&self) -> Result<usize> {
        let all = self.get_all_bundled_media().await?;
        let mut pruned = 0;
        for record in all {
            if !Path::new(&record.bundle_path).exists() {
                self.delete_bundled_media_by_id(record.id).await?;
                pruned += 1;
            }
        }
        Ok(pruned)
    }
}
