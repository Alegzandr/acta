//! Storage spike for ADR 0003. Throwaway code: proves the claims, nothing else.
//!
//! `acta-storage-spike run` orchestrates everything and prints a report.
//! The other subcommands are child-process roles spawned by `run`.
//! The key is never passed on the command line: every process reads the keychain itself.

use rusqlite::{Connection, ErrorCode, OpenFlags, TransactionBehavior, backup};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SERVICE: &str = "acta-spike";
const USER: &str = "db-key-v1";
const MARKER: &str = "ACTA-PLAINTEXT-MARKER-Alexandre-7f3a9c";

type Res<T> = Result<T, Box<dyn std::error::Error>>;

// ---------- key management ----------

fn entry() -> Res<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, USER)?)
}

/// `Ok(None)` means "the keychain works and has no entry", which is distinct from a keychain error.
fn load_key() -> Res<Option<[u8; 32]>> {
    // Stored as 64 hex characters: some Secret Service backends (KDE Wallet) only hold UTF-8 text.
    match entry()?.get_password() {
        Ok(text) => {
            let bytes: Vec<u8> = (0..text.len() / 2)
                .filter_map(|i| u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok())
                .collect();
            Ok(Some(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| "keychain entry is not a 256-bit hex key")?,
            ))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn store_key(key: &[u8; 32]) -> Res<()> {
    Ok(entry()?.set_password(&hex(key))?)
}

fn random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).expect("OS RNG");
    key
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------- open ----------

#[derive(Debug)]
enum OpenError {
    /// The database file exists but the keychain has no key for it.
    KeyMissing,
    /// The database file exists and a key was supplied, but it does not decrypt the file.
    KeyWrongOrCorrupt,
    Other(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::KeyMissing => {
                write!(f, "database exists but its key is not in the keychain")
            }
            OpenError::KeyWrongOrCorrupt => write!(f, "the key does not decrypt the database"),
            OpenError::Other(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for OpenError {}

fn other(e: impl std::fmt::Display) -> OpenError {
    OpenError::Other(e.to_string())
}

/// Opens an existing or new database file with a raw 256-bit key and sets the pragmas.
fn open_with_key(path: &Path, key: &[u8; 32], create: bool) -> Result<Connection, OpenError> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let conn = Connection::open_with_flags(path, flags).map_err(other)?;
    // Raw key: 64 hex digits, no PBKDF2. Must be the first statement on the connection.
    // SQLCipher logs decryption failures to stderr by default; the caller reports them instead.
    conn.execute_batch("PRAGMA cipher_log_level = NONE;")
        .map_err(other)?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex(key)))
        .map_err(other)?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(other)?;
    // First real read: this is where a wrong key surfaces as SQLITE_NOTADB.
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => {}
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == ErrorCode::NotADatabase => {
            return Err(OpenError::KeyWrongOrCorrupt);
        }
        Err(e) => return Err(other(e)),
    }
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;",
    )
    .map_err(other)?;
    Ok(conn)
}

#[derive(Debug, PartialEq)]
enum Opened {
    Created,
    Existing,
}

/// The only place allowed to create a database or a key. Never creates over an existing file.
fn open_or_init(path: &Path) -> Result<(Connection, Opened), OpenError> {
    // Serialises first-run initialisation between processes (GUI and MCP starting together).
    let lock = fs::File::create(path.with_extension("init.lock")).map_err(other)?;
    lock.lock().map_err(other)?;

    let key = load_key().map_err(other)?;
    let exists = path.exists();
    let result = match (exists, key) {
        (true, None) => Err(OpenError::KeyMissing),
        (true, Some(key)) => open_with_key(path, &key, false).map(|c| (c, Opened::Existing)),
        (false, existing_key) => {
            let key = match existing_key {
                Some(key) => key,
                None => {
                    // Persist the key and read it back before any data is written with it.
                    let key = random_key();
                    store_key(&key).map_err(other)?;
                    if load_key().map_err(other)? != Some(key) {
                        return Err(OpenError::Other(
                            "keychain did not round-trip the key".into(),
                        ));
                    }
                    key
                }
            };
            open_with_key(path, &key, true).map(|c| (c, Opened::Created))
        }
    };
    lock.unlock().map_err(other)?;
    result
}

// ---------- schema: hand-rolled user_version ladder ----------

const MIGRATIONS: &[&str] = &[
    // 1: every record has a stable uuid, updated_at and a soft-delete marker (ADR 0006).
    "CREATE TABLE topic (
        id INTEGER PRIMARY KEY,
        uuid TEXT NOT NULL UNIQUE,
        title TEXT NOT NULL,
        body TEXT NOT NULL,
        writer INTEGER NOT NULL,
        seq INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        deleted_at INTEGER
     ) STRICT;",
    // 2: full-text index kept in sync by triggers.
    "CREATE VIRTUAL TABLE topic_fts USING fts5(
        title, body, content='topic', content_rowid='id',
        tokenize='unicode61 remove_diacritics 2');
     CREATE TRIGGER topic_ai AFTER INSERT ON topic BEGIN
        INSERT INTO topic_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
     END;
     CREATE TRIGGER topic_ad AFTER DELETE ON topic BEGIN
        INSERT INTO topic_fts(topic_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
     END;
     CREATE TRIGGER topic_au AFTER UPDATE ON topic BEGIN
        INSERT INTO topic_fts(topic_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
        INSERT INTO topic_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
     END;",
];

fn migrate(conn: &mut Connection) -> Res<i64> {
    // IMMEDIATE: two processes starting together cannot both run the same step.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
    }
    tx.commit()?;
    Ok(MIGRATIONS.len() as i64)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn new_uuid() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).unwrap();
    hex(&b)
}

fn insert(conn: &mut Connection, title: &str, body: &str, writer: i64, seq: i64) -> Res<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = now_ms();
    tx.execute(
        "INSERT INTO topic (uuid, title, body, writer, seq, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        (new_uuid(), title, body, writer, seq, now),
    )?;
    tx.commit()?;
    Ok(())
}

// ---------- child roles ----------

fn child_writer(db: &Path, writer: i64, n: i64) -> Res<()> {
    let key = load_key()?.ok_or("no key")?;
    let mut conn = open_with_key(db, &key, false)?;
    for seq in 0..n {
        insert(
            &mut conn,
            "Concurrent write",
            &format!("writer {writer} row {seq}"),
            writer,
            seq,
        )?;
    }
    Ok(())
}

/// Writes `n` rows `gap_ms` apart and prints the wall-clock time of each commit.
fn child_pinger(db: &Path, n: i64, gap_ms: u64) -> Res<()> {
    let key = load_key()?.ok_or("no key")?;
    let mut conn = open_with_key(db, &key, false)?;
    println!("ready");
    for seq in 0..n {
        std::thread::sleep(Duration::from_millis(gap_ms));
        insert(&mut conn, "Ping", "external write", 9000, seq)?;
        println!("{}", now_ns());
    }
    Ok(())
}

/// What a hook pays: keychain fetch + open + one query. Prints the internal split in microseconds.
fn child_cold(db: &Path) -> Res<()> {
    let t0 = Instant::now();
    let key = load_key()?.ok_or("no key")?;
    let t1 = Instant::now();
    let conn = open_with_key(db, &key, false)?;
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM topic WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    let t2 = Instant::now();
    println!("{} {} {}", (t1 - t0).as_micros(), (t2 - t1).as_micros(), n);
    Ok(())
}

fn spawn(args: &[&str]) -> Res<std::process::Child> {
    Ok(Command::new(std::env::current_exe()?)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()?)
}

// ---------- helpers for the report ----------

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn db_files(db: &Path) -> Vec<PathBuf> {
    let name = db.file_name().unwrap().to_string_lossy().to_string();
    ["", "-wal", "-shm"]
        .iter()
        .map(|s| db.with_file_name(format!("{name}{s}")))
        .collect()
}

fn stats(mut v: Vec<f64>) -> String {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    format!(
        "n={} min={:.1} median={:.1} mean={:.1} p95={:.1} max={:.1}",
        v.len(),
        v[0],
        v[v.len() / 2],
        mean,
        v[(v.len() * 95 / 100).min(v.len() - 1)],
        v[v.len() - 1]
    )
}

fn check(name: &str, ok: bool) {
    println!("  [{}] {name}", if ok { "PASS" } else { "FAIL" });
    if !ok {
        FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
static FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// ---------- the run ----------

fn run() -> Res<()> {
    let dir = std::env::temp_dir().join(format!("acta-spike-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    let db = dir.join("acta.db");
    let db_s = db.to_string_lossy().to_string();
    let _ = entry()?.delete_credential();

    if let Some(p) = directories::ProjectDirs::from("", "", "Acta") {
        println!(
            "production data dir would be: {}",
            p.data_local_dir().display()
        );
    }

    println!("\n(a) create encrypted DB, key generated and stored in the OS keychain");
    let (mut conn, opened) = open_or_init(&db)?;
    println!(
        "  engine: SQLite {} / SQLCipher {} / provider {} {}",
        conn.query_row("SELECT sqlite_version()", [], |r| r.get::<_, String>(0))?,
        conn.query_row("PRAGMA cipher_version", [], |r| r.get::<_, String>(0))?,
        conn.query_row("PRAGMA cipher_provider", [], |r| r.get::<_, String>(0))?,
        conn.query_row("PRAGMA cipher_provider_version", [], |r| r
            .get::<_, String>(0))?,
    );
    check("first open creates the database", opened == Opened::Created);
    check("user_version ladder applied", migrate(&mut conn)? == 2);
    insert(&mut conn, "Marker topic", MARKER, 0, 0)?;
    drop(conn);
    let (mut conn, opened) = open_or_init(&db)?;
    check(
        "second open finds the key in the keychain and reuses the file",
        opened == Opened::Existing,
    );
    let key = load_key()?.ok_or("key vanished")?;

    println!("\n(b) no plaintext on disk (db, -wal, -shm), WAL not yet checkpointed");
    insert(&mut conn, "Second marker", MARKER, 0, 1)?;
    let mut total = 0;
    let mut leaked = false;
    for f in db_files(&db) {
        let bytes = fs::read(&f)?;
        total += bytes.len();
        let leak = contains(&bytes, MARKER.as_bytes())
            || contains(&bytes, b"SQLite format 3")
            || contains(&bytes, b"CREATE TABLE")
            || contains(&bytes, b"topic_fts");
        println!(
            "  {} {} bytes, plaintext found: {leak}",
            f.file_name().unwrap().to_string_lossy(),
            bytes.len()
        );
        leaked |= leak;
    }
    check(
        &format!("marker, header and schema text absent from {total} bytes"),
        !leaked,
    );
    let wal_len = fs::metadata(&db_files(&db)[1])?.len();
    check("the WAL held data during the scan", wal_len > 0);
    // Control: the same scan does find the marker in an unencrypted database.
    let plain = dir.join("plain.db");
    {
        let c = Connection::open(&plain)?;
        c.execute_batch("CREATE TABLE t (body TEXT);")?;
        c.execute("INSERT INTO t VALUES (?1)", [MARKER])?;
    }
    check(
        "control: the scan finds the marker in a plaintext database",
        contains(&fs::read(&plain)?, MARKER.as_bytes()),
    );

    println!("\n(c) wrong key and missing key");
    let wrong = open_with_key(&db, &random_key(), false);
    println!("  wrong key -> {:?}", wrong.as_ref().err());
    check(
        "wrong key fails with KeyWrongOrCorrupt",
        matches!(wrong, Err(OpenError::KeyWrongOrCorrupt)),
    );
    let nokey = Connection::open(&db)?.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    });
    println!(
        "  no key    -> {:?}",
        nokey.as_ref().err().map(|e| e.to_string())
    );
    check("no key fails", nokey.is_err());
    drop(conn);
    // Key lost: the app must refuse and must not touch the file.
    let before = fs::read(&db)?;
    entry()?.delete_credential()?;
    let lost = open_or_init(&db);
    println!("  key deleted from keychain -> {:?}", lost.as_ref().err());
    check(
        "existing file + no key is reported as KeyMissing",
        matches!(lost, Err(OpenError::KeyMissing)),
    );
    check(
        "the unreadable file was left byte-for-byte intact",
        fs::read(&db)? == before,
    );
    check("no new key was created", load_key()?.is_none());
    store_key(&key)?;
    let (mut conn, _) = open_or_init(&db)?;

    println!(
        "\n(d) concurrent writers in separate processes (WAL, busy_timeout 5000, BEGIN IMMEDIATE)"
    );
    let (writers, per) = (4i64, 250i64);
    let base: i64 = conn.query_row("SELECT count(*) FROM topic", [], |r| r.get(0))?;
    let t = Instant::now();
    let mut children = Vec::new();
    for w in 1..=writers {
        children.push(spawn(&["writer", &db_s, &w.to_string(), &per.to_string()])?);
    }
    for seq in 0..per {
        insert(
            &mut conn,
            "Concurrent write",
            &format!("parent row {seq}"),
            0,
            100 + seq,
        )?;
    }

    println!("\n(h) consistent snapshot taken while those writers are running");
    let snap_vacuum = dir.join("snapshot-vacuum.db");
    let snap_backup = dir.join("snapshot-backup.db");
    let during_before: i64 = conn.query_row("SELECT count(*) FROM topic", [], |r| r.get(0))?;
    let tv = Instant::now();
    let vacuum = conn.execute("VACUUM INTO ?1", [snap_vacuum.to_string_lossy()]);
    let vacuum_ms = tv.elapsed().as_secs_f64() * 1e3;
    let tb = Instant::now();
    let backup_res = (|| -> Res<()> {
        let mut dst = open_with_key(&snap_backup, &key, true)?;
        let b = backup::Backup::new(&conn, &mut dst)?;
        // One step for the whole file: a single read transaction, so writers cannot restart it.
        b.step(-1)?;
        Ok(())
    })();
    let backup_ms = tb.elapsed().as_secs_f64() * 1e3;
    let during_after: i64 = conn.query_row("SELECT count(*) FROM topic", [], |r| r.get(0))?;

    let mut ok = true;
    for c in &mut children {
        ok &= c.wait()?.success();
    }
    let elapsed = t.elapsed();
    let total_rows: i64 = conn.query_row("SELECT count(*) FROM topic", [], |r| r.get(0))?;
    let distinct: i64 = conn.query_row(
        "SELECT count(*) FROM (SELECT DISTINCT writer, seq FROM topic WHERE writer > 0)",
        [],
        |r| r.get(0),
    )?;
    let expected = base + (writers + 1) * per;
    println!(
        "  {} processes x {per} single-row transactions in {:.0} ms ({:.0} commits/s)",
        writers + 1,
        elapsed.as_secs_f64() * 1e3,
        ((writers + 1) * per) as f64 / elapsed.as_secs_f64()
    );
    check(
        "all writer processes exited cleanly (no SQLITE_BUSY surfaced)",
        ok,
    );
    check(
        &format!("no lost writes: {total_rows} rows, expected {expected}"),
        total_rows == expected,
    );
    check(
        "every (writer, seq) pair present once",
        distinct == writers * per,
    );

    for (name, path, res, ms) in [
        (
            "VACUUM INTO",
            &snap_vacuum,
            vacuum.map(|_| ()).map_err(|e| e.to_string()),
            vacuum_ms,
        ),
        (
            "backup API",
            &snap_backup,
            backup_res.map_err(|e| e.to_string()),
            backup_ms,
        ),
    ] {
        println!("  {name}: {res:?} in {ms:.1} ms");
        if res.is_err() {
            check(&format!("{name} produced a snapshot"), false);
            continue;
        }
        let bytes = fs::read(path)?;
        let s = open_with_key(path, &key, false);
        check(
            &format!("{name}: snapshot opens with the same key"),
            s.is_ok(),
        );
        if let Ok(s) = s {
            let rows: i64 = s.query_row("SELECT count(*) FROM topic", [], |r| r.get(0))?;
            let integrity: String = s.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            let marker: i64 = s.query_row(
                "SELECT count(*) FROM topic WHERE body = ?1",
                [MARKER],
                |r| r.get(0),
            )?;
            let fts: i64 = s.query_row(
                "SELECT count(*) FROM topic_fts WHERE topic_fts MATCH 'marker'",
                [],
                |r| r.get(0),
            )?;
            println!(
                "    rows={rows} (live db had {during_before}..{during_after} around the snapshot), integrity_check={integrity}"
            );
            check(
                &format!("{name}: data present and consistent"),
                integrity == "ok"
                    && marker == 2
                    && fts == 2
                    && rows >= base
                    && rows <= during_after,
            );
        }
        check(
            &format!("{name}: snapshot is encrypted ({} bytes)", bytes.len()),
            !contains(&bytes, MARKER.as_bytes()) && !contains(&bytes, b"SQLite format 3"),
        );
        check(
            &format!("{name}: snapshot does not open with a wrong key"),
            open_with_key(path, &random_key(), false).is_err(),
        );
    }

    println!("\n(e) change notification: reader polls PRAGMA data_version");
    let tp = Instant::now();
    let polls = 20_000;
    for _ in 0..polls {
        conn.pragma_query_value(None, "data_version", |r| r.get::<_, i64>(0))?;
    }
    println!(
        "  cost of one poll: {:.2} us",
        tp.elapsed().as_secs_f64() * 1e6 / polls as f64
    );
    let v0: i64 = conn.pragma_query_value(None, "data_version", |r| r.get(0))?;
    insert(&mut conn, "Own write", "x", 0, 9999)?;
    let v1: i64 = conn.pragma_query_value(None, "data_version", |r| r.get(0))?;
    check(
        "own commits do not change data_version (no self-wakeups)",
        v0 == v1,
    );
    for interval_ms in [100u64, 25] {
        let n = 20;
        let mut child = spawn(&["pinger", &db_s, &n.to_string(), "180"])?;
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        lines.next();
        let (tx, rx) = std::sync::mpsc::channel::<u128>();
        let reader = std::thread::spawn(move || {
            for l in lines.map_while(Result::ok) {
                let _ = tx.send(l.trim().parse().unwrap());
            }
        });
        let mut last: i64 = conn.pragma_query_value(None, "data_version", |r| r.get(0))?;
        let mut detected = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(15);
        while detected.len() < n && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(interval_ms));
            let v: i64 = conn.pragma_query_value(None, "data_version", |r| r.get(0))?;
            if v != last {
                last = v;
                detected.push(now_ns());
            }
        }
        child.wait()?;
        reader.join().unwrap();
        let commits: Vec<u128> = rx.try_iter().collect();
        check(
            &format!("poll every {interval_ms} ms: all {n} external commits detected"),
            detected.len() == n && commits.len() == n,
        );
        let lat: Vec<f64> = commits
            .iter()
            .zip(&detected)
            .map(|(c, d)| (*d as f64 - *c as f64) / 1e6)
            .collect();
        println!(
            "  poll every {interval_ms} ms: commit-to-detection latency ms: {}",
            stats(lat)
        );
    }

    println!("\n(f) FTS5 inside the encrypted database");
    insert(
        &mut conn,
        "Décision de stockage",
        "Alexandre chose an encrypted database for the timeline",
        0,
        10_000,
    )?;
    let hit: (String, String) = conn.query_row(
        "SELECT t.title, snippet(topic_fts, 1, '[', ']', '...', 6)
         FROM topic_fts JOIN topic t ON t.id = topic_fts.rowid
         WHERE topic_fts MATCH 'decision encrypt*' AND t.deleted_at IS NULL
         ORDER BY rank LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    println!("  match: {hit:?}");
    check(
        "MATCH with prefix query, diacritics folding, snippet() and rank",
        hit.0 == "Décision de stockage",
    );
    conn.execute(
        "UPDATE topic SET deleted_at = ?1, updated_at = ?1 WHERE title = 'Décision de stockage'",
        [now_ms()],
    )?;
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM topic_fts JOIN topic t ON t.id = topic_fts.rowid
         WHERE topic_fts MATCH 'decision' AND t.deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    check("soft-deleted rows are filtered out of search", n == 0);

    println!("\n(g) cold start of a short-lived process (spawn + keychain + open + one query)");
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    let (mut wall, mut kc, mut op) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..30 {
        let t = Instant::now();
        let out = Command::new(std::env::current_exe()?)
            .args(["cold", &db_s])
            .output()?;
        let w = t.elapsed().as_secs_f64() * 1e3;
        let s = String::from_utf8_lossy(&out.stdout);
        let p: Vec<f64> = s.split_whitespace().map(|x| x.parse().unwrap()).collect();
        if i == 0 {
            println!(
                "  first run: wall {w:.1} ms, keychain {:.2} ms, open+query {:.2} ms",
                p[0] / 1e3,
                p[1] / 1e3
            );
            continue;
        }
        wall.push(w);
        kc.push(p[0] / 1e3);
        op.push(p[1] / 1e3);
    }
    println!("  process wall time ms:      {}", stats(wall));
    println!("  keychain fetch ms:         {}", stats(kc));
    println!("  open + key + 1 query ms:   {}", stats(op));
    // What the raw key saves: the same open with a passphrase runs PBKDF2 (256,000 iterations).
    let kdf = dir.join("kdf.db");
    {
        let c = Connection::open(&kdf)?;
        c.execute_batch("PRAGMA key = 'a passphrase'; CREATE TABLE t (x);")?;
    }
    let t = Instant::now();
    {
        let c = Connection::open(&kdf)?;
        c.execute_batch("PRAGMA key = 'a passphrase';")?;
        c.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })?;
    }
    println!(
        "  for comparison, open with a passphrase (PBKDF2): {:.1} ms",
        t.elapsed().as_secs_f64() * 1e3
    );

    drop(conn);
    if std::env::var_os("ACTA_SPIKE_KEEP").is_some() {
        // Lets a binary built with the other crypto provider open this database (`cold`, then `cleanup`).
        println!(
            "
kept: {}",
            db.display()
        );
        return Ok(());
    }
    entry()?.delete_credential()?;
    check(
        "spike credential deleted from the keychain",
        load_key()?.is_none(),
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    let res = match arg(1) {
        "run" => run(),
        "writer" => child_writer(
            Path::new(arg(2)),
            arg(3).parse().unwrap(),
            arg(4).parse().unwrap(),
        ),
        "pinger" => child_pinger(
            Path::new(arg(2)),
            arg(3).parse().unwrap(),
            arg(4).parse().unwrap(),
        ),
        "cold" => child_cold(Path::new(arg(2))),
        "cleanup" => (|| {
            entry()?.delete_credential()?;
            Ok(fs::remove_dir_all(
                Path::new(arg(2)).parent().ok_or("no parent")?,
            )?)
        })(),
        _ => Err("usage: acta-storage-spike run".into()),
    };
    if let Err(e) = res {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
    if FAILED.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("\nSOME CHECKS FAILED");
        std::process::exit(1);
    }
    if arg(1) == "run" {
        println!("\nALL CHECKS PASSED");
    }
}
