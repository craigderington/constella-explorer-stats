use loco_rs::prelude::*;

use crate::{
    data::explorer::Settings,
    dtos::{
        chain::{BlockDto, SnapshotDto, StatusDto},
        common::Page,
    },
    models::_entities::{chain_blocks, chain_snapshots},
};

/// Public, read-only: chain status is public data, and the only writers are
/// the `chain_poll` task and its scheduler entry.
#[debug_handler]
async fn status(State(ctx): State<AppContext>) -> Result<Json<StatusDto>> {
    let settings = Settings::from_context(&ctx)?;
    let latest = chain_snapshots::Model::latest(&ctx.db).await?;
    Ok(Json(StatusDto::from_latest(latest.as_ref(), &settings)))
}

#[debug_handler]
async fn snapshots(
    State(ctx): State<AppContext>,
    Query(pagination): Query<query::PaginationQuery>,
) -> Result<Json<Page<SnapshotDto>>> {
    let res = query::paginate(
        &ctx.db,
        chain_snapshots::Entity::find().order_by_desc(chain_snapshots::Column::Id),
        None,
        &pagination,
    )
    .await?;

    Ok(Json(Page::from_query(res)))
}

#[debug_handler]
async fn blocks(
    State(ctx): State<AppContext>,
    Query(pagination): Query<query::PaginationQuery>,
) -> Result<Json<Page<BlockDto>>> {
    let res = query::paginate(
        &ctx.db,
        chain_blocks::Entity::find().order_by_desc(chain_blocks::Column::Height),
        None,
        &pagination,
    )
    .await?;

    Ok(Json(Page::from_query(res)))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/chain")
        .add("/status", get(status))
        .add("/snapshots", get(snapshots))
        .add("/blocks", get(blocks))
}
