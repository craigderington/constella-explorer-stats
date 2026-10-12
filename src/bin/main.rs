use loco_rs::cli;
use loco_sample::app::App;
use migration::Migrator;

// Loco does not read `.env` itself. Load it before the async runtime (and its
// worker threads) exist, because it mutates the process environment. Real
// environment variables always win, so production is unaffected.
fn main() -> loco_rs::Result<()> {
    dotenvy::dotenv().ok();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(cli::main::<App, Migrator>())
}
