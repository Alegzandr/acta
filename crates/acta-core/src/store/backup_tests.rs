//! Backups: round trip with attached files, wrong passphrase, damaged and
//! truncated files, older and foreign payloads, the payload cap, and the
//! passphrase rules.

use super::*;
use crate::model::{AttachOwner, Author, Status};
use crate::store::test_support::{self, TestStore};
use crate::store::{AttachSource, NewTopic};

const H: Author = Author::Human;

fn pass(text: &str) -> Passphrase {
    Passphrase::new(text.to_owned())
}

/// A store with a project, a topic, a note and a 1.5 MiB attached file.
fn filled() -> TestStore {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let p = s.create_project("Alexandre's garden", "", H).unwrap();
    s.create_topic(
        NewTopic {
            project: p,
            epic: None,
            title: "Plant the hedge".into(),
            description: "Ask Alexandre about the boxwood".into(),
            status: Status::Active,
        },
        H,
    )
    .unwrap();
    let n = s.add_note(p, "Order 40 plants", H).unwrap();
    let data: Vec<u8> = (0..1_536 * 1024).map(|i| (i * 7 % 251) as u8).collect();
    s.attach(
        AttachOwner::Note(n),
        "plan.bin",
        AttachSource::Bytes(data),
        H,
        &mut |_, _| {},
    )
    .unwrap();
    ts
}

fn backup_of(ts: &TestStore, dir: &Path, passphrase: &str) -> std::path::PathBuf {
    let path = dir.join("acta-2026-11-22.acta-backup");
    let file = ts.store.export(None).unwrap();
    file.save_backup(&path, &pass(passphrase)).unwrap();
    path
}

#[test]
fn a_backup_round_trips_with_its_files_and_holds_no_plaintext() {
    let ts = filled();
    let dir = tempfile::tempdir().unwrap();
    let path = backup_of(&ts, dir.path(), "correct horse battery");
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"age-encryption.org/v1\n-> scrypt "));
    assert!(is_backup(&path).unwrap());
    for needle in ["Alexandre", "hedge", "boxwood", "acta", "plan.bin"] {
        assert!(
            !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "{needle} in the backup"
        );
    }
    // No temporary file is left next to it.
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

    let back = ExportFile::read_backup(&path, &pass("correct horse battery")).unwrap();
    let mut now = ts.store.export(None).unwrap();
    // Only the export time differs from the export the backup holds.
    now.exported_at = back.exported_at;
    assert_eq!(back, now);
    assert_eq!(back.blobs.len(), 1);

    let mut other = test_support::open();
    let rep = other.store.import(&back, false).unwrap();
    assert_eq!(rep.attachments.created, 1);
    assert_eq!(rep.projects.created, 1);
    let mut again = other.store.export(None).unwrap();
    again.exported_at = back.exported_at;
    assert_eq!(
        again.to_json().unwrap(),
        back.to_json().unwrap(),
        "export, import, export again: identical"
    );
}

#[test]
fn the_wrong_passphrase_is_refused_before_anything_is_read() {
    let ts = filled();
    let dir = tempfile::tempdir().unwrap();
    let path = backup_of(&ts, dir.path(), "correct horse battery");
    assert!(matches!(
        ExportFile::read_backup(&path, &pass("correct horse batterY")),
        Err(Error::WrongPassphrase)
    ));
}

#[test]
fn a_damaged_or_truncated_backup_is_refused_whole() {
    let ts = filled();
    let dir = tempfile::tempdir().unwrap();
    let path = backup_of(&ts, dir.path(), "correct horse battery");
    let good = std::fs::read(&path).unwrap();
    let p = pass("correct horse battery");
    let damaged = dir.path().join("damaged.acta-backup");

    // One bit flipped in the middle of the payload.
    let mut flipped = good.clone();
    let at = flipped.len() / 2;
    flipped[at] ^= 0x01;
    std::fs::write(&damaged, &flipped).unwrap();
    assert!(matches!(
        ExportFile::read_backup(&damaged, &p),
        Err(Error::BackupDamaged)
    ));

    // Cut at a chunk boundary (the last chunk missing) and inside a chunk.
    for keep in [good.len() - 100, good.len() / 3, 200] {
        std::fs::write(&damaged, &good[..keep]).unwrap();
        assert!(
            matches!(
                ExportFile::read_backup(&damaged, &p),
                Err(Error::BackupDamaged)
            ),
            "cut at {keep}"
        );
    }

    // The header's MAC no longer matches.
    let mut header = good.clone();
    let line = header.iter().position(|&b| b == b'\n').unwrap() + 10;
    header[line] ^= 0x20;
    std::fs::write(&damaged, &header).unwrap();
    assert!(ExportFile::read_backup(&damaged, &p).is_err());

    // Empty file: not a backup.
    std::fs::write(&damaged, b"").unwrap();
    assert!(!is_backup(&damaged).unwrap());
}

#[test]
fn a_plaintext_export_or_another_file_is_not_a_backup() {
    let ts = filled();
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("acta-2026-11-20.acta.json");
    std::fs::write(&old, ts.store.export(None).unwrap().to_json().unwrap()).unwrap();
    assert!(!is_backup(&old).unwrap());
    match ExportFile::read_backup(&old, &pass("correct horse battery")) {
        Err(Error::Invalid(m)) => assert_eq!(m, "this is not an Acta backup"),
        other => panic!("{other:?}"),
    }
}

/// An age file with any payload, made with this module's work factor.
fn encrypt_raw(path: &Path, passphrase: &str, payload: &[u8]) {
    let mut r = age::scrypt::Recipient::new(SecretString::from(passphrase.to_owned()));
    r.set_work_factor(LOG_N);
    let e = age::Encryptor::with_recipients(std::iter::once(&r as &dyn age::Recipient)).unwrap();
    let mut w = e.wrap_output(File::create(path).unwrap()).unwrap();
    w.write_all(payload).unwrap();
    w.finish().unwrap();
}

#[test]
fn older_newer_and_foreign_payloads_say_so() {
    let ts = filled();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.acta-backup");
    let p = pass("correct horse battery");
    let mut v: serde_json::Value =
        serde_json::from_str(&ts.store.export(None).unwrap().to_json().unwrap()).unwrap();

    // A version 4 document (the last plaintext format): refused, clearly.
    v["version"] = 4.into();
    encrypt_raw(&path, "correct horse battery", v.to_string().as_bytes());
    assert!(matches!(
        ExportFile::read_backup(&path, &p),
        Err(Error::FormatTooOld {
            found: 4,
            oldest: 5
        })
    ));

    // A newer one whose fields no longer parse still says it is newer.
    v["version"] = 9.into();
    v["projects"] = "changed".into();
    encrypt_raw(&path, "correct horse battery", v.to_string().as_bytes());
    assert!(matches!(
        ExportFile::read_backup(&path, &p),
        Err(Error::FormatTooNew { found: 9, .. })
    ));

    // Something else encrypted with age.
    encrypt_raw(&path, "correct horse battery", b"just a note");
    assert!(matches!(
        ExportFile::read_backup(&path, &p),
        Err(Error::Json(_))
    ));
    encrypt_raw(
        &path,
        "correct horse battery",
        b"{\"format\":\"x\",\"version\":5}",
    );
    assert!(matches!(
        ExportFile::read_backup(&path, &p),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn the_payload_is_capped_while_it_is_decrypted() {
    let ts = filled();
    let dir = tempfile::tempdir().unwrap();
    let path = backup_of(&ts, dir.path(), "correct horse battery");
    let p = pass("correct horse battery");
    // The file is 2 MiB and more once decrypted: a 1 MiB cap stops
    // reading the payload at 1 MiB, before parsing it whole.
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > 2 * 1024 * 1024);
    match read_backup_within(&path, &p, 1024 * 1024) {
        Err(Error::FileTooLarge { limit, .. }) => assert_eq!(limit, 1024 * 1024),
        other => panic!("{other:?}"),
    }
    // Checked on the file's size first when that is enough.
    match read_backup_within(&path, &p, 64 * 1024) {
        Err(Error::FileTooLarge { size: s, .. }) => assert_eq!(s, size),
        other => panic!("{other:?}"),
    }
    let mut capped = Capped {
        inner: &b"0123456789"[..],
        left: 4,
        over: false,
    };
    let mut out = Vec::new();
    assert!(capped.read_to_end(&mut out).is_err());
    assert!(capped.over);
    assert_eq!(out, b"0123");
}

#[test]
fn a_backup_asking_for_too_much_work_is_refused_before_scrypt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hostile.acta-backup");
    // A real header names its log N in the stanza: rewrite it to 30
    // (2^30 x 1 KiB of memory). The MAC no longer matches, but the work
    // factor is checked first.
    encrypt_raw(&path, "correct horse battery", b"{}");
    let bytes = std::fs::read(&path).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let line_end = text.find('\n').unwrap() + 1;
    let stanza_end = line_end + text[line_end..].find('\n').unwrap();
    let stanza = &text[line_end..stanza_end];
    let (head, _) = stanza.rsplit_once(' ').unwrap();
    let mut forged = bytes[..line_end].to_vec();
    forged.extend_from_slice(format!("{head} 30").as_bytes());
    forged.extend_from_slice(&bytes[stanza_end..]);
    std::fs::write(&path, forged).unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(
        ExportFile::read_backup(&path, &pass("correct horse battery")),
        Err(Error::Invalid(_))
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn a_short_passphrase_is_refused_and_nothing_is_written() {
    let ts = filled();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("short.acta-backup");
    let file = ts.store.export(None).unwrap();
    assert!(matches!(
        file.save_backup_new(&path, &pass("seven77")),
        Err(Error::PassphraseTooShort { min: 8 })
    ));
    assert!(!path.exists());
    assert!(matches!(
        file.save_backup(&path, &pass("seven77")),
        Err(Error::PassphraseTooShort { min: 8 })
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    // The CLI never overwrites.
    std::fs::write(&path, b"keep").unwrap();
    assert!(file.save_backup_new(&path, &pass("eight888")).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    // Eight characters, accents counted once each.
    assert_eq!(pass("éléphant").chars(), 8);
    assert_eq!(format!("{:?}", pass("éléphant")), "Passphrase(..)");
}

#[test]
fn the_strength_hint_is_a_hint() {
    assert_eq!(strength(""), Strength::TooShort);
    assert_eq!(strength("abc1234"), Strength::TooShort);
    assert_eq!(strength("12345678"), Strength::Weak);
    assert_eq!(strength("aaaaaaaaaaaaaaaaaaaa"), Strength::Weak);
    assert_eq!(strength("alexandre"), Strength::Weak);
    assert_eq!(strength("Password1!"), Strength::Fair);
    assert_eq!(strength("correct horse battery staple"), Strength::Strong);
    assert_eq!(strength("Tr0ub4dor&3-Alexandre"), Strength::Strong);
}
