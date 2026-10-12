use axum::http::header;
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

const FAVICON: &str = include_str!("../../assets/static/favicon.svg");

/// The site icon, cacheable for a day.
async fn favicon() -> Result<Response> {
    axum::http::Response::builder()
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(axum::body::Body::from(FAVICON))
        .map_err(|e| Error::string(&e.to_string()))
}

/// Browsers ask for `/favicon.ico` unprompted; point them at the SVG.
async fn favicon_ico() -> Result<Response> {
    format::redirect("/favicon.svg")
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/", get(index))
        .add("/favicon.svg", get(favicon))
        .add("/favicon.ico", get(favicon_ico))
}
