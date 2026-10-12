use super::_entities::chain_alerts::Column;
pub use super::_entities::chain_alerts::{ActiveModel, Entity, Model};
use super::chain_snapshots::ChainAlert;
use loco_rs::prelude::*;
use sea_orm::ActiveValue::Set;
use sea_orm::{entity::prelude::*, IntoActiveModel, QueryOrder};
pub type ChainAlerts = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

impl Model {
    /// Undelivered transition emails, oldest first.
    ///
    /// # Errors
    /// On database failure.
    pub async fn pending(db: &DatabaseConnection) -> ModelResult<Vec<Self>> {
        Ok(Entity::find()
            .filter(Column::DeliveredAt.is_null())
            .order_by_asc(Column::Id)
            .all(db)
            .await?)
    }

    /// Mark a queued email delivered after the mailer accepts it.
    ///
    /// # Errors
    /// On database failure.
    pub async fn mark_delivered(self, db: &DatabaseConnection) -> ModelResult<Self> {
        let mut active = self.into_active_model();
        active.delivered_at = Set(Some(chrono::Utc::now().into()));
        Ok(active.update(db).await?)
    }

    /// Persist alert events in the same transaction as their snapshot.
    ///
    /// # Errors
    /// On database failure.
    pub async fn enqueue_for_snapshot<C>(
        db: &C,
        snapshot_id: i64,
        alerts: &[ChainAlert],
    ) -> ModelResult<()>
    where
        C: ConnectionTrait,
    {
        for alert in alerts {
            ActiveModel {
                alert_type: Set(alert.alert_type().to_string()),
                detail: Set(alert.detail()),
                chain_snapshots: Set(snapshot_id),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
        Ok(())
    }
}

// implement your write-oriented logic here
impl ActiveModel {}

// implement your custom finders, selectors oriented logic here
impl Entity {}
