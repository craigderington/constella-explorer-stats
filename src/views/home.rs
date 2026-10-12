//! View model for the HTML index page: everything is formatted here so the
//! template only has to print strings.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    dtos::chain::StatusDto,
    models::_entities::{chain_blocks, chain_snapshots},
};

/// Atoms per coin (1e-8).
const ATOMS_PER_COIN: i64 = 100_000_000;

#[derive(Serialize)]
pub struct IndexView {
    /// `healthy`, `degraded`, `stale` or `nodata` — doubles as a CSS class.
    pub state: &'static str,
    pub headline: &'static str,
    pub stale_after_secs: String,
    pub stats: Vec<Stat>,
    pub blocks: Vec<BlockRow>,
    pub polls: Vec<PollRow>,
}

#[derive(Serialize)]
pub struct Stat {
    pub label: &'static str,
    pub value: String,
}

#[derive(Serialize)]
pub struct BlockRow {
    pub height: String,
    pub time: String,
    pub tuple: &'static str,
    pub bits: String,
    pub miner: String,
    pub certified: bool,
    pub main_chain: bool,
}

#[derive(Serialize)]
pub struct PollRow {
    pub at: String,
    pub ok: bool,
    pub height: String,
    pub ledger: String,
    pub peer: String,
    pub error: String,
}

fn time(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn opt_time<Tz: chrono::TimeZone>(t: Option<DateTime<Tz>>) -> String {
    t.map_or_else(|| "—".to_string(), |t| time(t.with_timezone(&Utc)))
}

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map_or_else(|| "—".to_string(), |v| v.to_string())
}

/// Group digits in thousands: `1234567` -> `1,234,567`.
fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

fn coins(atoms: Option<i64>) -> String {
    atoms.map_or_else(
        || "—".to_string(),
        |a| {
            format!(
                "{}.{:02}",
                grouped(a / ATOMS_PER_COIN),
                (a % ATOMS_PER_COIN).abs() / (ATOMS_PER_COIN / 100)
            )
        },
    )
}

fn short(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > 16 {
        let head: String = chars[..8].iter().collect();
        let tail: String = chars[chars.len() - 6..].iter().collect();
        format!("{head}…{tail}")
    } else {
        s.to_string()
    }
}

const fn tuple_name(size: i64) -> &'static str {
    match size {
        5 => "quintuplet",
        6 => "sextuplet",
        _ => "constellation",
    }
}

fn stats_for(l: &chain_snapshots::Model) -> Vec<Stat> {
    vec![
        Stat {
            label: "Network",
            value: opt(l.network.as_deref()),
        },
        Stat {
            label: "Height",
            value: l.height.map_or_else(|| "—".into(), grouped),
        },
        Stat {
            label: "Chain ID",
            value: opt(l.chain_id.as_deref()),
        },
        Stat {
            label: "Tip",
            value: l.tip.as_deref().map_or_else(|| "—".into(), short),
        },
        Stat {
            label: "Difficulty (bits)",
            value: opt(l.bits),
        },
        Stat {
            label: "Blocks",
            value: l.blocks.map_or_else(|| "—".into(), grouped),
        },
        Stat {
            label: "Transactions",
            value: opt(l.txs),
        },
        Stat {
            label: "Miners",
            value: opt(l.miners),
        },
        Stat {
            label: "Shares / min",
            value: l
                .shares_per_min
                .map_or_else(|| "—".into(), |v| format!("{v:.1}")),
        },
        Stat {
            label: "Node peer",
            value: match l.peer {
                Some(true) => "connected".into(),
                Some(false) => "lost".into(),
                None => "—".into(),
            },
        },
        Stat {
            label: "Ledger check",
            value: opt(l.ledger_check.as_deref()),
        },
        Stat {
            label: "Science escrow",
            value: coins(l.escrow),
        },
        Stat {
            label: "Science paid",
            value: coins(l.sci_paid),
        },
        Stat {
            label: "Last block",
            value: opt_time(l.last_block_at),
        },
        Stat {
            label: "Last polled",
            value: time(l.created_at.with_timezone(&Utc)),
        },
    ]
}

impl IndexView {
    #[must_use]
    pub fn new(
        status: &StatusDto,
        latest: Option<&chain_snapshots::Model>,
        blocks: Vec<chain_blocks::Model>,
        polls: Vec<chain_snapshots::Model>,
    ) -> Self {
        let (state, headline) = match latest {
            None => ("nodata", "No data yet"),
            Some(_) if status.healthy => ("healthy", "Chain healthy"),
            Some(_) if status.stale => ("stale", "Data is stale"),
            Some(_) => ("degraded", "Chain degraded"),
        };

        let tiles = latest.map_or_else(Vec::new, stats_for);

        Self {
            state,
            headline,
            stale_after_secs: status.stale_after_secs.to_string(),
            stats: tiles,
            blocks: blocks
                .into_iter()
                .map(|b| BlockRow {
                    height: grouped(b.height),
                    time: time(b.block_time.with_timezone(&Utc)),
                    tuple: tuple_name(b.tuple),
                    bits: b.bits.to_string(),
                    miner: short(&b.miner),
                    certified: b.certified,
                    main_chain: b.main_chain,
                })
                .collect(),
            polls: polls
                .into_iter()
                .map(|p| PollRow {
                    at: time(p.created_at.with_timezone(&Utc)),
                    ok: p.ok,
                    height: p.height.map_or_else(|| "—".into(), grouped),
                    ledger: opt(p.ledger_check),
                    peer: p.peer.map_or_else(|| "—".into(), |v| v.to_string()),
                    error: p.error.unwrap_or_default(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_digits() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(144_762), "144,762");
        assert_eq!(grouped(-1_234_567), "-1,234,567");
    }

    #[test]
    fn formats_atoms_as_coins() {
        assert_eq!(coins(Some(31_500_000_000)), "315.00");
        assert_eq!(coins(Some(150_000_000)), "1.50");
        assert_eq!(coins(None), "—");
    }

    #[test]
    fn shortens_long_identifiers_only() {
        assert_eq!(short("abc"), "abc");
        assert_eq!(
            short("9ddb19dc0da58f6fdab63bb464ea79d453b4a7ace36ef421ff71fcc674ec3557"),
            "9ddb19dc…ec3557"
        );
    }
}
