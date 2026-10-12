use loco_rs::prelude::*;

use crate::{
    data::explorer::Settings,
    dtos::chain::StatusDto,
    models::{chain_blocks, chain_snapshots},
    views::home::IndexView,
};

const BLOCKS_SHOWN: u64 = 25;
const POLLS_SHOWN: u64 = 20;

/// The public stats page. Dynamic values reach the template as plain strings
/// and are escaped there (`views::template` does not autoescape).
#[debug_handler]
async fn index(State(ctx): State<AppContext>) -> Result<Response> {
    let settings = Settings::from_context(&ctx)?;
    let latest = chain_snapshots::Model::latest(&ctx.db).await?;
    let status = StatusDto::from_latest(latest.as_ref(), &settings);
    let blocks = chain_blocks::Model::recent(&ctx.db, BLOCKS_SHOWN).await?;
    let polls = chain_snapshots::Model::recent(&ctx.db, POLLS_SHOWN).await?;
    let view = IndexView::new(&status, latest.as_ref(), blocks, polls);
    format::template(include_str!("../../assets/views/index.html"), view)
}

pub fn routes() -> Routes {
    Routes::new().add("/", get(index))
}
