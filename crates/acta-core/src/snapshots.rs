//! Automatic snapshots (ADR 0006): consistent, encrypted copies of the
//! database in `snapshots/` next to it, taken with `VACUUM INTO` and renamed
//! into place (ADR 0003).
//!
//! Taken before every migration, import and snapshot restore, and once per
//! day of use. Retention: the 7 newest daily snapshots, then the newest of
//! each of the 4 weeks before them, the 3 newest `restore` snapshots, and
//! the newest of every other kind. A snapshot being restored is never
//! removed by retention.

use crate::error::Result;
use crate::fsutil;
use crate::time;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

pub const SNAPSHOT_DIR: &str = "snapshots";
const KEEP_DAILY: usize = 7;
const KEEP_WEEKLY: usize = 4;
const KEEP_RESTORE: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SnapshotKind {
    Daily,
    Migration,
    Import,
    Restore,
    Manual,
}

impl SnapshotKind {
    const ALL: [SnapshotKind; 5] = [
        SnapshotKind::Daily,
        SnapshotKind::Migration,
        SnapshotKind::Import,
        SnapshotKind::Restore,
        SnapshotKind::Manual,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SnapshotKind::Daily => "daily",
            SnapshotKind::Migration => "migration",
            SnapshotKind::Import => "import",
            SnapshotKind::Restore => "restore",
            SnapshotKind::Manual => "manual",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub path: PathBuf,
    pub kind: SnapshotKind,
    pub taken_at: i64,
}

/// `acta-20261121T101502123Z-daily.db`
fn file_name(kind: SnapshotKind, taken_at: i64) -> String {
    format!("acta-{}-{}.db", time::file_stamp(taken_at), kind.as_str())
}

fn parse_name(name: &str) -> Option<(SnapshotKind, i64)> {
    let rest = name.strip_prefix("acta-")?.strip_suffix(".db")?;
    let (stamp, kind) = rest.split_once('-')?;
    Some((SnapshotKind::parse(kind)?, time::parse_file_stamp(stamp)?))
}

/// Takes a snapshot of the database `conn` is open on, then applies retention.
pub(crate) fn take(conn: &Connection, data_dir: &Path, kind: SnapshotKind) -> Result<Snapshot> {
    take_protecting(conn, data_dir, kind, None)
}

/// [`take`], with a snapshot retention must leave in place (the one about
/// to be restored).
pub(crate) fn take_protecting(
    conn: &Connection,
    data_dir: &Path,
    kind: SnapshotKind,
    protect: Option<&Path>,
) -> Result<Snapshot> {
    let dir = data_dir.join(SNAPSHOT_DIR);
    fsutil::create_private_dir(&dir)?;
    // Two snapshots in the same millisecond get distinct names.
    let mut taken_at = time::now_ms();
    while dir.join(file_name(kind, taken_at)).exists() {
        taken_at += 1;
    }
    let path = dir.join(file_name(kind, taken_at));
    let tmp = dir.join(format!(".tmp-{}", file_name(kind, taken_at)));
    let _ = fs::remove_file(&tmp);
    let result = conn
        .execute("VACUUM INTO ?1", [tmp.to_string_lossy()])
        .map_err(crate::Error::from)
        .and_then(|_| fsutil::make_private(&tmp).map_err(crate::Error::from))
        .and_then(|_| fs::rename(&tmp, &path).map_err(crate::Error::from));
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let snapshot = Snapshot {
        path,
        kind,
        taken_at,
    };
    for old in expired(&list(data_dir)?) {
        if protect.is_some_and(|p| same_file(p, &old.path)) {
            continue;
        }
        let _ = fs::remove_file(&old.path);
    }
    Ok(snapshot)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Every snapshot, newest first.
pub fn list(data_dir: &Path) -> Result<Vec<Snapshot>> {
    let dir = data_dir.join(SNAPSHOT_DIR);
    let mut out = Vec::new();
    let read = match fs::read_dir(&dir) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for item in read {
        let item = item?;
        let name = item.file_name();
        if let Some((kind, taken_at)) = name.to_str().and_then(parse_name) {
            out.push(Snapshot {
                path: item.path(),
                kind,
                taken_at,
            });
        }
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.taken_at));
    Ok(out)
}

/// The snapshots retention drops, from a list sorted newest first.
pub(crate) fn expired(all: &[Snapshot]) -> Vec<Snapshot> {
    let mut keep = vec![false; all.len()];
    let mut daily_seen = 0;
    let mut weeks: Vec<i64> = Vec::new();
    let mut other_seen: Vec<SnapshotKind> = Vec::new();
    let mut restore_seen = 0;
    for (i, s) in all.iter().enumerate() {
        if s.kind == SnapshotKind::Daily {
            if daily_seen < KEEP_DAILY {
                daily_seen += 1;
                keep[i] = true;
            } else {
                let week = time::week_number(s.taken_at);
                if !weeks.contains(&week) && weeks.len() < KEEP_WEEKLY {
                    weeks.push(week);
                    keep[i] = true;
                }
            }
        } else if s.kind == SnapshotKind::Restore {
            if restore_seen < KEEP_RESTORE {
                restore_seen += 1;
                keep[i] = true;
            }
        } else if !other_seen.contains(&s.kind) {
            other_seen.push(s.kind);
            keep[i] = true;
        }
    }
    all.iter()
        .zip(keep)
        .filter(|(_, k)| !k)
        .map(|(s, _)| s.clone())
        .collect()
}

/// Whether a daily snapshot is due: none taken yet on today's UTC date.
pub(crate) fn daily_due(all: &[Snapshot], now: i64) -> bool {
    !all.iter().any(|s| {
        s.kind == SnapshotKind::Daily && time::day_number(s.taken_at) == time::day_number(now)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::MS_PER_DAY;

    fn snap(kind: SnapshotKind, day: i64) -> Snapshot {
        Snapshot {
            path: PathBuf::from(file_name(kind, day * MS_PER_DAY)),
            kind,
            taken_at: day * MS_PER_DAY + 1000,
        }
    }

    #[test]
    fn names_round_trip() {
        let t = 20_778 * MS_PER_DAY + 5;
        let name = file_name(SnapshotKind::Migration, t);
        assert_eq!(name, "acta-20261121T000000005Z-migration.db");
        assert_eq!(parse_name(&name), Some((SnapshotKind::Migration, t)));
        assert_eq!(parse_name("acta.db"), None);
        assert_eq!(parse_name(".tmp-acta-20261121T000000005Z-daily.db"), None);
    }

    #[test]
    fn retention_keeps_7_daily_4_weekly_and_the_last_of_each_kind() {
        // 60 consecutive days of daily snapshots, newest first, plus others.
        let today = 20_778; // a Saturday
        let mut all: Vec<Snapshot> = (0..60)
            .map(|d| snap(SnapshotKind::Daily, today - d))
            .collect();
        all.push(snap(SnapshotKind::Migration, today - 1));
        all.push(snap(SnapshotKind::Migration, today - 30));
        all.push(snap(SnapshotKind::Import, today - 40));
        for d in 50..55 {
            all.push(snap(SnapshotKind::Restore, today - d));
        }
        all.sort_by_key(|s| std::cmp::Reverse(s.taken_at));

        let dropped = expired(&all);
        let kept: Vec<&Snapshot> = all.iter().filter(|s| !dropped.contains(s)).collect();
        let daily: Vec<i64> = kept
            .iter()
            .filter(|s| s.kind == SnapshotKind::Daily)
            .map(|s| time::day_number(s.taken_at))
            .collect();
        assert_eq!(daily.len(), 7 + 4);
        assert_eq!(
            &daily[..7],
            &(0..7).map(|d| today - d).collect::<Vec<_>>()[..]
        );
        let weeks: Vec<i64> = daily[7..]
            .iter()
            .map(|d| time::week_number(d * MS_PER_DAY))
            .collect();
        let mut distinct = weeks.clone();
        distinct.dedup();
        assert_eq!(distinct.len(), 4);
        assert!(kept.contains(&&snap(SnapshotKind::Migration, today - 1)));
        assert!(!kept.contains(&&snap(SnapshotKind::Migration, today - 30)));
        assert!(kept.contains(&&snap(SnapshotKind::Import, today - 40)));
        let restores: Vec<_> = kept
            .iter()
            .filter(|s| s.kind == SnapshotKind::Restore)
            .collect();
        assert_eq!(restores.len(), 3);
        assert!(restores.contains(&&&snap(SnapshotKind::Restore, today - 50)));
    }

    #[test]
    fn a_daily_snapshot_is_due_once_per_utc_day() {
        let today = 20_778;
        let all = vec![snap(SnapshotKind::Daily, today - 1)];
        assert!(daily_due(&all, today * MS_PER_DAY + 5));
        let all = vec![snap(SnapshotKind::Daily, today)];
        assert!(!daily_due(&all, today * MS_PER_DAY + 50_000));
        assert!(daily_due(&[], 0));
    }
}
