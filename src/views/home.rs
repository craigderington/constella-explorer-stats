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
/// How many of the newest blocks are drawn as constellations in the hero.
const SKY_BLOCKS: usize = 10;

#[derive(Serialize)]
pub struct IndexView {
    /// `healthy`, `degraded`, `stale` or `nodata` — doubles as a CSS class.
    pub state: &'static str,
    pub headline: &'static str,
    /// Why the chain is not healthy; empty when it is.
    pub reason: String,
    pub stale_after_secs: String,
    pub height: String,
    pub network: String,
    pub polled_ago: String,
    pub groups: Vec<Group>,
    pub sky: Vec<SkyBlock>,
    pub blocks: Vec<BlockRow>,
    pub beats: Vec<Beat>,
    pub polls: Vec<PollRow>,
}

#[derive(Serialize)]
pub struct Group {
    pub title: &'static str,
    pub stats: Vec<Stat>,
}

#[derive(Serialize)]
pub struct Stat {
    pub label: &'static str,
    pub value: String,
}

/// A block's prime constellation: stars and the line joining them, in a
/// 120x80 viewBox.
#[derive(Serialize)]
pub struct Glyph {
    pub path: String,
    pub stars: Vec<Star>,
}

#[derive(Serialize)]
pub struct Star {
    pub x: String,
    pub y: String,
    pub r: String,
}

#[derive(Serialize)]
pub struct SkyBlock {
    pub height: String,
    pub tuple: &'static str,
    pub newest: bool,
    pub glyph: Glyph,
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
    pub glyph: Glyph,
}

/// One tick of the poll heartbeat strip.
#[derive(Serialize)]
pub struct Beat {
    pub ok: bool,
    pub title: String,
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

/// "12 seconds ago", "5 minutes ago", "3 hours ago", "2 days ago".
fn ago(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = now.signed_duration_since(then).num_seconds().max(0);
    let (n, unit) = match secs {
        0..=59 => (secs, "second"),
        60..=3_599 => (secs / 60, "minute"),
        3_600..=86_399 => (secs / 3_600, "hour"),
        _ => (secs / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
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

/// Admissible prime-tuple offsets: the star spacing of a mined constellation.
const fn pattern(size: i64) -> &'static [u32] {
    match size {
        5 => &[0, 4, 6, 10, 12],
        6 => &[0, 4, 6, 10, 12, 16],
        _ => &[0, 2, 6, 8, 12],
    }
}

/// splitmix64 over an FNV-1a seed of the block hash: stable, so a block always
/// draws the same constellation.
fn rng(seed: &str) -> impl FnMut() -> f64 {
    let mut state = seed.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        // top 53 bits -> [0, 1)
        #[allow(clippy::cast_precision_loss)]
        let unit = (z >> 11) as f64 / (1_u64 << 53) as f64;
        unit
    }
}

fn glyph(block_id: &str, tuple: i64) -> Glyph {
    let offsets = pattern(tuple);
    let span = f64::from(*offsets.last().unwrap_or(&1));
    let mut next = rng(block_id);
    let points: Vec<(f64, f64, f64)> = offsets
        .iter()
        .map(|&o| {
            let x = 10.0 + 100.0 * f64::from(o) / span;
            let y = 60.0_f64.mul_add(next(), 10.0);
            let r = 1.7_f64.mul_add(next(), 1.5);
            (x, y, r)
        })
        .collect();
    let path = points
        .iter()
        .enumerate()
        .map(|(i, (x, y, _))| format!("{}{x:.1} {y:.1}", if i == 0 { "M" } else { "L" }))
        .collect::<Vec<_>>()
        .join(" ");
    Glyph {
        path,
        stars: points
            .iter()
            .map(|(x, y, r)| Star {
                x: format!("{x:.1}"),
                y: format!("{y:.1}"),
                r: format!("{r:.1}"),
            })
            .collect(),
    }
}

fn stats_groups(l: &chain_snapshots::Model, now: DateTime<Utc>) -> Vec<Group> {
    let big = |v: Option<i64>| v.map_or_else(|| "—".to_string(), grouped);
    vec![
        Group {
            title: "Chain",
            stats: vec![
                Stat {
                    label: "Chain ID",
                    value: opt(l.chain_id.as_deref()),
                },
                Stat {
                    label: "Tip",
                    value: l.tip.as_deref().map_or_else(|| "—".into(), short),
                },
                Stat {
                    label: "Difficulty",
                    value: l.bits.map_or_else(|| "—".into(), |b| format!("{b} bits")),
                },
                Stat {
                    label: "Blocks mined",
                    value: big(l.blocks),
                },
                Stat {
                    label: "Transactions",
                    value: big(l.txs),
                },
            ],
        },
        Group {
            title: "Mining",
            stats: vec![
                Stat {
                    label: "Miners",
                    value: big(l.miners),
                },
                Stat {
                    label: "Shares per minute",
                    value: l
                        .shares_per_min
                        .map_or_else(|| "—".into(), |v| format!("{v:.1}")),
                },
                Stat {
                    label: "Last block",
                    value: l
                        .last_block_at
                        .map_or_else(|| "—".into(), |t| ago(t.with_timezone(&Utc), now)),
                },
                Stat {
                    label: "Science escrow",
                    value: coins(l.escrow),
                },
                Stat {
                    label: "Science paid",
                    value: coins(l.sci_paid),
                },
            ],
        },
        Group {
            title: "Explorer",
            stats: vec![
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
                    label: "Ledger checked at",
                    value: big(l.ledger_check_height),
                },
                Stat {
                    label: "Its own update",
                    value: opt_time(l.stats_updated_at),
                },
                Stat {
                    label: "Last polled",
                    value: time(l.created_at.with_timezone(&Utc)),
                },
            ],
        },
    ]
}

/// Why the chain is not healthy, in one sentence ("" when it is).
fn reason_for(status: &StatusDto, l: &chain_snapshots::Model) -> String {
    if status.healthy {
        String::new()
    } else if !l.ok {
        format!(
            "The last poll failed: {}",
            l.error.as_deref().unwrap_or("no error recorded")
        )
    } else if status.stale {
        format!(
            "The latest poll, or the explorer's own view, is older than {} seconds.",
            status.stale_after_secs
        )
    } else if status.ledger_disagreement {
        format!(
            "The explorer's ledger check reports {:?} instead of \"ok\".",
            l.ledger_check.as_deref().unwrap_or("nothing")
        )
    } else {
        "The explorer has lost its peer connection to its node.".to_string()
    }
}

impl IndexView {
    #[must_use]
    pub fn new(
        status: &StatusDto,
        latest: Option<&chain_snapshots::Model>,
        blocks: Vec<chain_blocks::Model>,
        polls: Vec<chain_snapshots::Model>,
    ) -> Self {
        let now = Utc::now();
        let (state, headline) = match latest {
            None => ("nodata", "No data yet"),
            Some(_) if status.healthy => ("healthy", "Chain healthy"),
            Some(l) if l.ok && status.stale => ("stale", "Data is stale"),
            Some(_) => ("degraded", "Chain degraded"),
        };

        let sky = blocks
            .iter()
            .take(SKY_BLOCKS)
            .enumerate()
            .map(|(i, b)| SkyBlock {
                height: grouped(b.height),
                tuple: tuple_name(b.tuple),
                newest: i == 0,
                glyph: glyph(&b.block_id, b.tuple),
            })
            .collect();

        Self {
            state,
            headline,
            reason: latest.map_or_else(String::new, |l| reason_for(status, l)),
            stale_after_secs: status.stale_after_secs.to_string(),
            height: latest
                .and_then(|l| l.height)
                .map_or_else(|| "—".into(), grouped),
            network: latest.and_then(|l| l.network.clone()).unwrap_or_default(),
            polled_ago: latest
                .map_or_else(String::new, |l| ago(l.created_at.with_timezone(&Utc), now)),
            groups: latest.map_or_else(Vec::new, |l| stats_groups(l, now)),
            sky,
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
                    glyph: glyph(&b.block_id, b.tuple),
                })
                .collect(),
            beats: polls
                .iter()
                .rev()
                .map(|p| Beat {
                    ok: p.ok,
                    title: format!(
                        "{}: {}",
                        time(p.created_at.with_timezone(&Utc)),
                        if p.ok { "ok" } else { "failed" }
                    ),
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

    #[test]
    fn describes_elapsed_time() {
        let now = Utc::now();
        let before = |s| now - chrono::Duration::seconds(s);
        assert_eq!(ago(before(1), now), "1 second ago");
        assert_eq!(ago(before(45), now), "45 seconds ago");
        assert_eq!(ago(before(120), now), "2 minutes ago");
        assert_eq!(ago(before(3_700), now), "1 hour ago");
        assert_eq!(ago(before(200_000), now), "2 days ago");
        assert_eq!(
            ago(now + chrono::Duration::seconds(30), now),
            "0 seconds ago"
        );
    }

    #[test]
    fn constellations_are_stable_and_match_the_tuple_size() {
        let a = glyph("99b66988", 5);
        let b = glyph("99b66988", 5);
        assert_eq!(a.path, b.path);
        assert_eq!(a.stars.len(), 5);
        assert_eq!(glyph("99b66988", 6).stars.len(), 6);
        assert_ne!(glyph("aaaa", 5).path, glyph("bbbb", 5).path);
    }
}
