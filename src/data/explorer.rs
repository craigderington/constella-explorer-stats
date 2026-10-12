//! Wire types for the Catasterism explorer JSON API (`/api/stats`,
//! `/api/blocks`), plus the companion's own typed settings.
//!
//! The explorer sends most `meta` values as JSON strings ("448", "true").
//! Conversion to typed values happens here, once, at the edge — the rest of
//! the app never sees a stringly-typed number.

use chrono::{DateTime, FixedOffset};
use loco_rs::prelude::*;
use serde::Deserialize;

/// Typed view of the `settings:` block in `config/<env>.yaml`.
#[derive(Clone, Deserialize)]
pub struct Settings {
    /// Base URL of the explorer, e.g. `https://explorer.catasterism.xyz`.
    pub explorer_url: String,
    /// Where chain-alert mail is delivered.
    pub alert_email: String,
    /// Mailgun API credentials and sending identity.
    pub mailgun: MailgunSettings,
    /// A snapshot older than this — by poll age *or* by the explorer's own
    /// `updated_at` age — counts as stale.
    #[serde(default = "default_stale_after_secs")]
    pub stale_after_secs: i64,
}

#[derive(Clone, Deserialize)]
pub struct MailgunSettings {
    #[serde(default, deserialize_with = "empty_if_null")]
    pub api_key: String,
    #[serde(default, deserialize_with = "empty_if_null")]
    pub domain: String,
    /// Optional sender; empty means `Catasterism Alerts <alerts@domain>`.
    #[serde(default, deserialize_with = "empty_if_null")]
    pub from: String,
    #[serde(default = "default_mailgun_api_base_url")]
    pub api_base_url: String,
}

/// An unset env var renders as an empty YAML value, which arrives as `null`.
fn empty_if_null<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn default_mailgun_api_base_url() -> String {
    "https://api.mailgun.net".to_string()
}

const fn default_stale_after_secs() -> i64 {
    300
}

impl Settings {
    /// # Errors
    /// When `settings:` is absent from the environment config or does not
    /// match this schema, including Mailgun configuration.
    pub fn from_context(ctx: &AppContext) -> Result<Self> {
        let raw = ctx
            .config
            .settings
            .clone()
            .ok_or_else(|| Error::string("missing `settings:` in config"))?;
        Ok(serde_json::from_value(raw)?)
    }

    /// Absolute `/api/stats` URL.
    #[must_use]
    pub fn stats_url(&self) -> String {
        format!("{}/api/stats", self.explorer_url.trim_end_matches('/'))
    }

    /// Absolute `/api/blocks` URL.
    #[must_use]
    pub fn blocks_url(&self) -> String {
        format!("{}/api/blocks", self.explorer_url.trim_end_matches('/'))
    }
}

/// The `/api/stats` payload.
#[derive(Debug, Deserialize)]
pub struct ExplorerStats {
    /// RFC 3339 timestamp of the most recent block.
    pub last_block: Option<String>,
    pub meta: StatsMeta,
    pub miners: Option<i64>,
    pub shares_per_min: Option<f64>,
}

/// The `meta` object of `/api/stats` — every value arrives as a string.
#[derive(Debug, Deserialize)]
pub struct StatsMeta {
    pub bits: Option<String>,
    pub blocks: Option<String>,
    pub chain_id: Option<String>,
    /// Explorer-vs-node ledger agreement: `"ok"` when they match. Anything
    /// else is a disagreement worth an alert.
    pub check: Option<String>,
    pub check_at: Option<String>,
    pub check_count: Option<String>,
    pub check_height: Option<String>,
    pub check_total: Option<String>,
    /// Science-lane escrow balance, in atoms (1e-8 coin).
    pub escrow: Option<String>,
    pub height: Option<String>,
    pub known: Option<String>,
    pub network: Option<String>,
    /// `"true"` while the explorer holds a live peer connection to its node.
    pub peer: Option<String>,
    pub sci_claims: Option<String>,
    pub sci_paid: Option<String>,
    /// Full block hash of the chain tip.
    pub tip: Option<String>,
    pub txs: Option<String>,
    /// RFC 3339 — when the explorer last updated its own view.
    pub updated_at: Option<String>,
}

fn parse_i64(s: Option<&str>) -> Option<i64> {
    s.and_then(|v| v.trim().parse().ok())
}

fn parse_bool(s: Option<&str>) -> Option<bool> {
    s?.trim().parse().ok()
}

fn parse_time(s: Option<&str>) -> Option<DateTime<FixedOffset>> {
    s.and_then(|v| DateTime::parse_from_rfc3339(v.trim()).ok())
}

impl StatsMeta {
    #[must_use]
    pub fn bits(&self) -> Option<i64> {
        parse_i64(self.bits.as_deref())
    }
    #[must_use]
    pub fn blocks(&self) -> Option<i64> {
        parse_i64(self.blocks.as_deref())
    }
    #[must_use]
    pub fn height(&self) -> Option<i64> {
        parse_i64(self.height.as_deref())
    }
    #[must_use]
    pub fn txs(&self) -> Option<i64> {
        parse_i64(self.txs.as_deref())
    }
    #[must_use]
    pub fn escrow(&self) -> Option<i64> {
        parse_i64(self.escrow.as_deref())
    }
    #[must_use]
    pub fn sci_paid(&self) -> Option<i64> {
        parse_i64(self.sci_paid.as_deref())
    }
    #[must_use]
    pub fn sci_claims(&self) -> Option<i64> {
        parse_i64(self.sci_claims.as_deref())
    }
    #[must_use]
    pub fn ledger_check_height(&self) -> Option<i64> {
        parse_i64(self.check_height.as_deref())
    }
    #[must_use]
    pub fn peer(&self) -> Option<bool> {
        parse_bool(self.peer.as_deref())
    }
    #[must_use]
    pub fn updated_at(&self) -> Option<DateTime<FixedOffset>> {
        parse_time(self.updated_at.as_deref())
    }
}

impl ExplorerStats {
    #[must_use]
    pub fn last_block_at(&self) -> Option<DateTime<FixedOffset>> {
        parse_time(self.last_block.as_deref())
    }
}

/// One entry of the `/api/blocks` array.
#[derive(Debug, Deserialize)]
pub struct ExplorerBlock {
    /// Full block hash — the dedup key.
    pub id: String,
    pub height: i64,
    /// RFC 3339 block time.
    pub time: String,
    /// Miner payout address (64-hex).
    pub miner: String,
    pub bits: i32,
    /// Constellation size: 5 = quintuplet, 6 = sextuplet.
    pub tuple: i32,
    #[serde(rename = "block")]
    pub is_block: bool,
    /// Re-checked with Baillie-PSW by the explorer, beyond consensus's Fermat.
    pub certified: bool,
    /// The block's prime, decimal.
    pub p: String,
    pub txs: i64,
    /// On the main chain (not an orphan).
    pub main: bool,
}

impl ExplorerBlock {
    #[must_use]
    pub fn block_time(&self) -> Option<DateTime<FixedOffset>> {
        DateTime::parse_from_rfc3339(self.time.trim()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed from a real `/api/stats` response (testnet, 2026-10-03).
    const STATS_JSON: &str = r#"{
        "last_block": "2026-10-03T21:28:35Z",
        "meta": {
            "bits": "448",
            "blocks": "1390",
            "chain_id": "a8f4562e57e74f9d",
            "check": "ok",
            "check_at": "2026-10-03T21:29:06Z",
            "check_count": "7",
            "check_height": "108597",
            "check_total": "7",
            "escrow": "31500000000",
            "height": "108615",
            "known": "127028",
            "network": "testnet",
            "peer": "true",
            "sci_claims": "710198",
            "sci_paid": "4833500000000",
            "tip": "9ddb19dc0da58f6fdab63bb464ea79d453b4a7ace36ef421ff71fcc674ec3557",
            "txs": "6",
            "updated_at": "2026-10-03T21:30:17Z"
        },
        "miners": 7,
        "shares_per_min": 16.4
    }"#;

    /// Trimmed from a real `/api/blocks` response.
    const BLOCKS_JSON: &str = r#"[{
        "id": "99b66988f45c42ea5b1994e9e76de3a74ba9512e61dc3d846d28f3daf78cff1b",
        "height": 108592,
        "time": "2026-10-03T21:28:35Z",
        "miner": "34d89b9ee084c78196b0ae1709dc08276a8ef90068149c1a7db451aa0280d84d",
        "bits": 448,
        "tuple": 5,
        "block": true,
        "certified": true,
        "p": "387251822093895454342874956580647339240251834954973327774977273789624977902314748030106247582092912584684878766099584021443805043905397",
        "txs": 0,
        "main": true
    }]"#;

    #[test]
    fn mailgun_settings_accept_null_values_from_unset_env_vars() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "explorer_url": "http://x",
            "alert_email": "a@b.c",
            "mailgun": { "api_key": "k", "domain": "d", "from": null }
        }))
        .unwrap();
        assert_eq!(settings.mailgun.from, "");
        assert_eq!(settings.mailgun.api_base_url, "https://api.mailgun.net");
    }

    #[test]
    fn parses_real_stats_payload() {
        let stats: ExplorerStats = serde_json::from_str(STATS_JSON).unwrap();
        assert_eq!(stats.meta.height(), Some(108_615));
        assert_eq!(stats.meta.bits(), Some(448));
        assert_eq!(stats.meta.blocks(), Some(1_390));
        assert_eq!(stats.meta.txs(), Some(6));
        assert_eq!(stats.meta.escrow(), Some(31_500_000_000));
        assert_eq!(stats.meta.sci_paid(), Some(4_833_500_000_000));
        assert_eq!(stats.meta.sci_claims(), Some(710_198));
        assert_eq!(stats.meta.ledger_check_height(), Some(108_597));
        assert_eq!(stats.meta.peer(), Some(true));
        assert_eq!(stats.meta.check.as_deref(), Some("ok"));
        assert_eq!(stats.meta.network.as_deref(), Some("testnet"));
        assert_eq!(stats.miners, Some(7));
        assert_eq!(stats.shares_per_min, Some(16.4));
        assert_eq!(
            stats.meta.updated_at().map(|t| t.timestamp()),
            Some(1_791_063_017) // 2026-10-03T21:30:17Z
        );
        assert_eq!(
            stats.last_block_at().map(|t| t.timestamp()),
            Some(1_791_062_915) // 2026-10-03T21:28:35Z
        );
    }

    #[test]
    fn parses_real_blocks_payload() {
        let blocks: Vec<ExplorerBlock> = serde_json::from_str(BLOCKS_JSON).unwrap();
        assert_eq!(blocks.len(), 1);
        let b = &blocks[0];
        assert_eq!(b.height, 108_592);
        assert_eq!(b.bits, 448);
        assert_eq!(b.tuple, 5);
        assert!(b.is_block && b.certified && b.main);
        assert!(b.p.starts_with("387251822093"));
        assert_eq!(b.block_time().map(|t| t.timestamp()), Some(1_791_062_915));
    }

    #[test]
    fn tolerates_missing_and_malformed_meta_values() {
        let stats: ExplorerStats =
            serde_json::from_str(r#"{ "meta": { "height": "not-a-number", "peer": "false" } }"#)
                .unwrap();
        assert_eq!(stats.meta.height(), None);
        assert_eq!(stats.meta.peer(), Some(false));
        assert_eq!(stats.meta.bits(), None);
        assert_eq!(stats.last_block_at(), None);

        let stats: ExplorerStats =
            serde_json::from_str(r#"{ "meta": { "peer": "unknown" } }"#).unwrap();
        assert_eq!(stats.meta.peer(), None);
    }
}
