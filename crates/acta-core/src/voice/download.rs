//! The model downloader (ADR 0014, 4). Entered only from a click that named
//! the size; never at startup, never in the background unasked, never two
//! at once in one models folder.
//!
//! The body streams through SHA-256 into `<file>.part`; a size or hash
//! mismatch deletes the part and fails. A part left by an interrupted
//! download is re-hashed and resumed with a `Range` request. Success is
//! `fsync`, then rename, then a small record of the file's size and
//! modification time, compared at each load ([`check_model`]): any
//! difference re-hashes before use.
//!
//! The model being replaced or deleted must not be in use: the caller
//! first calls `take::Controller::release_engine`, which ends the worker
//! process, and touches the file only when it returned `true`.
//!
//! Redirects are followed by hand, at most five, each target checked
//! against the [`Policy`]: HTTPS only in the app. Model files are public
//! weights, not user data: plaintext on disk is fine (ADR 0014, 4). Nothing
//! is written outside the models folder.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::models::{FREE_SPACE_MARGIN, ModelFile};
use crate::fsutil;

const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
const CHUNK: usize = 64 * 1024;

/// Which URL schemes a request may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schemes {
    HttpsOnly,
    /// Plain HTTP to `127.0.0.1`, `localhost` or `[::1]` as well: tests
    /// against a local server.
    HttpsOrLoopback,
}

/// The schemes allowed for the first URL and for every redirect target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub start: Schemes,
    pub redirects: Schemes,
}

impl Policy {
    /// The app's: HTTPS everywhere.
    pub const RELEASE: Policy = Policy {
        start: Schemes::HttpsOnly,
        redirects: Schemes::HttpsOnly,
    };
}

/// The file expected, byte for byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expected<'a> {
    pub file: &'a str,
    pub size: u64,
    /// SHA-256, lowercase hex.
    pub sha256: &'a str,
}

impl<'a> From<&'a ModelFile> for Expected<'a> {
    fn from(m: &'a ModelFile) -> Self {
        Expected {
            file: m.file,
            size: m.size,
            sha256: m.sha256,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Bytes on disk so far (a resumed part counts from its length).
    Bytes { done: u64, total: u64 },
    /// The body is complete; the hash is being compared.
    Checking,
}

/// Why a download failed. Messages carry no URL and no path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadError {
    /// Another download runs into the same folder.
    Busy,
    /// The download would leave less than 1 GB free.
    NoSpace {
        needed: u64,
        free: u64,
    },
    /// A URL or redirect the policy refuses (plain HTTP in the app).
    Insecure,
    TooManyRedirects,
    /// The server answered with this status.
    Status(u16),
    /// The connection failed or was cut; the part is kept for a resume.
    Network,
    /// The file did not match its size or hash; the part was deleted.
    Mismatch,
    /// Cancelled; the part was deleted.
    Cancelled,
    /// The disk refused a write or a rename.
    Disk(io::ErrorKind),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Busy => f.write_str("a download is already running"),
            DownloadError::NoSpace { needed, free } => {
                write!(
                    f,
                    "not enough free space: {needed} bytes needed, {free} free"
                )
            }
            DownloadError::Insecure => f.write_str("refused a non-HTTPS address"),
            DownloadError::TooManyRedirects => f.write_str("too many redirects"),
            DownloadError::Status(s) => write!(f, "the server answered {s}"),
            DownloadError::Network => f.write_str("the connection failed"),
            DownloadError::Mismatch => f.write_str("the file did not match"),
            DownloadError::Cancelled => f.write_str("cancelled"),
            DownloadError::Disk(kind) => write!(f, "disk error: {kind:?}"),
        }
    }
}

impl std::error::Error for DownloadError {}

/// One download, as the UI asks for it after a click.
pub struct Request<'a> {
    pub url: String,
    pub expected: Expected<'a>,
    /// The models folder; created if missing.
    pub dir: &'a Path,
    pub policy: Policy,
    /// Free bytes on the disk holding a path ([`free_space`] in the app).
    pub free_space: &'a dyn Fn(&Path) -> io::Result<u64>,
    pub cancel: &'a AtomicBool,
    pub progress: &'a mut dyn FnMut(Progress),
}

/// Folders with a download running.
static RUNNING: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

struct Running(PathBuf);

impl Running {
    fn claim(dir: &Path) -> Option<Running> {
        let mut set = RUNNING.lock().unwrap_or_else(|p| p.into_inner());
        let set = set.get_or_insert_with(HashSet::new);
        set.insert(dir.to_path_buf())
            .then(|| Running(dir.to_path_buf()))
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let mut set = RUNNING.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(set) = set.as_mut() {
            set.remove(&self.0);
        }
    }
}

fn part_path(dir: &Path, file: &str) -> PathBuf {
    dir.join(format!("{file}.part"))
}

fn record_path(dir: &Path, file: &str) -> PathBuf {
    dir.join(format!("{file}.checked"))
}

fn disk(e: io::Error) -> DownloadError {
    DownloadError::Disk(e.kind())
}

/// Downloads `req.expected` into `req.dir`. Returns the model's path.
pub fn download(mut req: Request<'_>) -> Result<PathBuf, DownloadError> {
    let _running = Running::claim(req.dir).ok_or(DownloadError::Busy)?;
    fsutil::create_private_dir(req.dir).map_err(disk)?;
    let part = part_path(req.dir, req.expected.file);
    match fetch(&mut req, &part) {
        Ok(()) => {
            let target = req.dir.join(req.expected.file);
            fs::rename(&part, &target).map_err(disk)?;
            sync_dir(req.dir);
            record(req.dir, req.expected.file).map_err(disk)?;
            Ok(target)
        }
        Err(e) => {
            if matches!(e, DownloadError::Cancelled | DownloadError::Mismatch) {
                let _ = fs::remove_file(&part);
            }
            Err(e)
        }
    }
}

fn fetch(req: &mut Request<'_>, part: &Path) -> Result<(), DownloadError> {
    let expected = req.expected;
    // A part longer than the file cannot be resumed.
    let mut have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have > expected.size {
        fs::remove_file(part).map_err(disk)?;
        have = 0;
    }
    let mut hasher = Sha256::new();
    if have > 0 {
        have = rehash(part, have, &mut hasher, req.cancel)?;
    }

    let free = (req.free_space)(req.dir).map_err(disk)?;
    let needed = expected.size - have + FREE_SPACE_MARGIN;
    if free < needed {
        return Err(DownloadError::NoSpace { needed, free });
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_connect(Some(TIMEOUT))
        .timeout_per_read(Some(TIMEOUT))
        .user_agent(format!("Acta/{}", crate::VERSION))
        .build()
        .into();

    let mut restarted = false;
    let response = 'request: loop {
        let mut url = req.url.clone();
        allowed(&url, req.policy.start)?;
        for _ in 0..=MAX_REDIRECTS {
            let mut call = agent.get(&url);
            if have > 0 {
                call = call.header("Range", format!("bytes={have}-"));
            }
            let response = call.call().map_err(|_| DownloadError::Network)?;
            let status = response.status().as_u16();
            match status {
                301 | 302 | 303 | 307 | 308 => {
                    let location = response
                        .headers()
                        .get("location")
                        .and_then(|v| v.to_str().ok())
                        .ok_or(DownloadError::Status(status))?;
                    url = resolve(&url, location).ok_or(DownloadError::Insecure)?;
                    allowed(&url, req.policy.redirects)?;
                }
                200 | 206 | 416 => {
                    let resumed =
                        status == 206 && content_range(&response) == Some((have, expected.size));
                    if have > 0 && !resumed && !restarted {
                        // The server ignored or refused the range (or the
                        // part was complete but stale): from the start.
                        restarted = true;
                        fs::remove_file(part).map_err(disk)?;
                        have = 0;
                        hasher = Sha256::new();
                        continue 'request;
                    }
                    if status != 200 && !resumed {
                        return Err(DownloadError::Status(status));
                    }
                    if status == 200
                        && let Some(len) = content_length(&response)
                        && len != expected.size
                    {
                        return Err(DownloadError::Mismatch);
                    }
                    break 'request response;
                }
                _ => return Err(DownloadError::Status(status)),
            }
        }
        return Err(DownloadError::TooManyRedirects);
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(part)
        .map_err(disk)?;
    let mut body = response.into_body().into_reader();
    let mut buf = vec![0u8; CHUNK];
    let mut done = have;
    (req.progress)(Progress::Bytes {
        done,
        total: expected.size,
    });
    loop {
        if req.cancel.load(Ordering::SeqCst) {
            return Err(DownloadError::Cancelled);
        }
        let n = match body.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {
                let _ = file.sync_all();
                return Err(DownloadError::Network);
            }
        };
        done += n as u64;
        if done > expected.size {
            return Err(DownloadError::Mismatch);
        }
        file.write_all(&buf[..n]).map_err(disk)?;
        hasher.update(&buf[..n]);
        (req.progress)(Progress::Bytes {
            done,
            total: expected.size,
        });
    }
    file.sync_all().map_err(disk)?;
    drop(file);
    if done < expected.size {
        // A clean end before the whole file: resumable.
        return Err(DownloadError::Network);
    }
    (req.progress)(Progress::Checking);
    if hex(&hasher.finalize()) != expected.sha256 {
        return Err(DownloadError::Mismatch);
    }
    Ok(())
}

/// Hashes the first `len` bytes of the part into `hasher`; returns the
/// bytes hashed (the part may be shorter than its metadata said).
fn rehash(
    part: &Path,
    len: u64,
    hasher: &mut Sha256,
    cancel: &AtomicBool,
) -> Result<u64, DownloadError> {
    let mut f = File::open(part).map_err(disk)?;
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    while done < len {
        if cancel.load(Ordering::SeqCst) {
            return Err(DownloadError::Cancelled);
        }
        let want = ((len - done) as usize).min(CHUNK);
        let n = f.read(&mut buf[..want]).map_err(disk)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
    }
    // Anything past what was hashed is cut, so the append continues at
    // `done`.
    drop(f);
    let f = OpenOptions::new().write(true).open(part).map_err(disk)?;
    f.set_len(done).map_err(disk)?;
    Ok(done)
}

fn content_length(response: &ureq::http::Response<ureq::Body>) -> Option<u64> {
    response
        .headers()
        .get("content-length")?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// `Content-Range: bytes <start>-<end>/<total>` as (start, total).
fn content_range(response: &ureq::http::Response<ureq::Body>) -> Option<(u64, u64)> {
    let v = response.headers().get("content-range")?.to_str().ok()?;
    let rest = v.trim().strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (start, _) = range.split_once('-')?;
    Some((start.trim().parse().ok()?, total.trim().parse().ok()?))
}

/// Whether `url` may be fetched under `schemes`.
fn allowed(url: &str, schemes: Schemes) -> Result<(), DownloadError> {
    let (scheme, authority, _) = split(url).ok_or(DownloadError::Insecure)?;
    let ok = match scheme.to_ascii_lowercase().as_str() {
        "https" => true,
        "http" => schemes == Schemes::HttpsOrLoopback && is_loopback(authority),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(DownloadError::Insecure)
    }
}

fn is_loopback(authority: &str) -> bool {
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => authority.split(':').next().unwrap_or(""),
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// `scheme://authority/path...` into its three parts.
fn split(url: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let cut = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(cut);
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some((scheme, authority, path))
}

/// A redirect's `Location` against the URL that gave it: absolute, or an
/// absolute path on the same origin. Anything else is refused.
fn resolve(base: &str, location: &str) -> Option<String> {
    if split(location).is_some() {
        return Some(location.to_string());
    }
    if location.starts_with('/') && !location.starts_with("//") {
        let (scheme, authority, _) = split(base)?;
        return Some(format!("{scheme}://{authority}{location}"));
    }
    None
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The hex SHA-256 of everything `r` yields (sha2 0.11 hashers are no
/// longer `io::Write`, so no `io::copy`).
fn sha256_hex(mut r: impl Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        match r.read(&mut buf) {
            Ok(0) => return Ok(hex(&hasher.finalize())),
            Ok(n) => hasher.update(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

fn sync_dir(_dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(_dir) {
        let _ = d.sync_all();
    }
}

fn stamp(path: &Path) -> io::Result<String> {
    let meta = fs::metadata(path)?;
    let modified = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Ok(format!("{} {}", meta.len(), modified.as_nanos()))
}

/// Records the size and modification time of a model just checked.
fn record(dir: &Path, file: &str) -> io::Result<()> {
    let stamp = stamp(&dir.join(file))?;
    fsutil::write_private_atomic(&record_path(dir, file), stamp.as_bytes())
}

/// A model on disk, as the UI and the worker's start see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelState {
    Missing,
    Ready(PathBuf),
    /// Present but not the expected file (`voice.error.model`, "Download
    /// again").
    Damaged,
}

/// Whether the model is on disk and intact. Cheap when its size and
/// modification time match the record; otherwise the file is hashed again
/// (and the record renewed when it matches).
pub fn check_model(dir: &Path, expected: Expected<'_>) -> ModelState {
    let path = dir.join(expected.file);
    let Ok(meta) = fs::metadata(&path) else {
        return ModelState::Missing;
    };
    if meta.len() != expected.size {
        return ModelState::Damaged;
    }
    let recorded = fs::read_to_string(record_path(dir, expected.file)).ok();
    if recorded.is_some() && recorded == stamp(&path).ok() {
        return ModelState::Ready(path);
    }
    let Ok(f) = File::open(&path) else {
        return ModelState::Damaged;
    };
    if sha256_hex(f).ok().as_deref() != Some(expected.sha256) {
        return ModelState::Damaged;
    }
    let _ = record(dir, expected.file);
    ModelState::Ready(path)
}

/// Deletes a model, its part and its record ("Delete model").
pub fn delete_model(dir: &Path, file: &str) -> io::Result<()> {
    for p in [dir.join(file), part_path(dir, file), record_path(dir, file)] {
        match fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Free bytes for this user on the disk holding `path`.
pub fn free_space(path: &Path) -> io::Result<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt as _;
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let mut free = 0u64;
        // SAFETY: a NUL-terminated path and a valid out pointer; the other
        // outputs are optional.
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(free)
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        let c = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        // SAFETY: a NUL-terminated path and a zeroed struct to fill.
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut stat) } != 0 {
            return Err(io::Error::last_os_error());
        }
        #[allow(clippy::unnecessary_cast)]
        Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;

    /// A few hundred KB of generated bytes and their hash: never a model.
    fn payload(len: usize) -> (Vec<u8>, String) {
        let mut seed = 0x9e37_79b9u32;
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed as u8
            })
            .collect();
        let sha = hex(&Sha256::digest(&bytes));
        (bytes, sha)
    }

    #[derive(Clone)]
    enum Behaviour {
        /// Serve the bytes, honouring `Range`.
        Serve(Arc<Vec<u8>>),
        /// The first request is cut after this many bytes; later ones are
        /// served whole (with `Range`).
        CutOnce(Arc<Vec<u8>>, usize),
        /// Redirect to this URL.
        Redirect(String),
        /// Serve the bytes slowly (for cancel).
        Slow(Arc<Vec<u8>>),
    }

    /// A local HTTP/1.1 server on a random port, one thread per connection.
    struct Server {
        url: String,
        /// The `Range` header of each request ("" when none).
        ranges: Arc<Mutex<Vec<String>>>,
    }

    impl Server {
        fn start(behaviour: Behaviour) -> Server {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/model.bin", listener.local_addr().unwrap());
            let ranges = Arc::new(Mutex::new(Vec::new()));
            let seen = Arc::clone(&ranges);
            std::thread::spawn(move || {
                let mut count = 0;
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { return };
                    count += 1;
                    let behaviour = behaviour.clone();
                    let seen = Arc::clone(&seen);
                    let first = count == 1;
                    std::thread::spawn(move || handle(stream, behaviour, seen, first));
                }
            });
            Server { url, ranges }
        }

        fn ranges(&self) -> Vec<String> {
            self.ranges.lock().unwrap().clone()
        }
    }

    fn handle(stream: TcpStream, behaviour: Behaviour, seen: Arc<Mutex<Vec<String>>>, first: bool) {
        let mut reader = io::BufReader::new(stream.try_clone().unwrap());
        let mut range = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line
                .to_ascii_lowercase()
                .strip_prefix("range:")
                .map(str::trim)
            {
                range = v.to_string();
            }
        }
        seen.lock().unwrap().push(range.clone());
        let mut out = stream;
        let serve = |out: &mut TcpStream, bytes: &[u8], limit: Option<usize>, slow: bool| {
            let start: usize = range
                .strip_prefix("bytes=")
                .and_then(|r| r.strip_suffix('-'))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let body = &bytes[start..];
            let head = if start > 0 {
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                    body.len(),
                    start,
                    bytes.len() - 1,
                    bytes.len()
                )
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
            };
            let _ = out.write_all(head.as_bytes());
            let body = &body[..limit.unwrap_or(body.len()).min(body.len())];
            for piece in body.chunks(8 * 1024) {
                if out.write_all(piece).is_err() {
                    return;
                }
                if slow {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            let _ = out.flush();
        };
        match behaviour {
            Behaviour::Serve(bytes) => serve(&mut out, &bytes, None, false),
            Behaviour::Slow(bytes) => serve(&mut out, &bytes, None, true),
            Behaviour::CutOnce(bytes, cut) => {
                serve(&mut out, &bytes, first.then_some(cut), false);
            }
            Behaviour::Redirect(to) => {
                let _ = write!(
                    out,
                    "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
        }
        let _ = out.shutdown(std::net::Shutdown::Both);
    }

    const LOCAL: Policy = Policy {
        start: Schemes::HttpsOrLoopback,
        redirects: Schemes::HttpsOrLoopback,
    };

    fn plenty(_: &Path) -> io::Result<u64> {
        Ok(u64::MAX / 2)
    }

    struct Setup {
        root: tempfile::TempDir,
        bytes: Vec<u8>,
        sha: String,
    }

    impl Setup {
        fn new(len: usize) -> Setup {
            let (bytes, sha) = payload(len);
            Setup {
                root: tempfile::tempdir().unwrap(),
                bytes,
                sha,
            }
        }

        fn dir(&self) -> PathBuf {
            self.root.path().join("models")
        }

        fn expected(&self) -> Expected<'_> {
            Expected {
                file: "ggml-test.bin",
                size: self.bytes.len() as u64,
                sha256: &self.sha,
            }
        }

        fn run(
            &self,
            url: &str,
            expected: Expected<'_>,
            policy: Policy,
            free: &dyn Fn(&Path) -> io::Result<u64>,
            cancel: &AtomicBool,
        ) -> (Result<PathBuf, DownloadError>, Vec<Progress>) {
            let mut seen = Vec::new();
            let mut progress = |p: Progress| seen.push(p);
            let dir = self.dir();
            let result = download(Request {
                url: url.to_string(),
                expected,
                dir: &dir,
                policy,
                free_space: free,
                cancel,
                progress: &mut progress,
            });
            (result, seen)
        }

        fn simple(&self, url: &str) -> Result<PathBuf, DownloadError> {
            self.run(
                url,
                self.expected(),
                LOCAL,
                &plenty,
                &AtomicBool::new(false),
            )
            .0
        }

        /// Everything under the temp root, relative: nothing may be
        /// outside `models/`.
        fn files(&self) -> Vec<String> {
            let mut all = Vec::new();
            for e in fs::read_dir(self.root.path()).unwrap() {
                let e = e.unwrap();
                all.push(e.file_name().to_string_lossy().into_owned());
            }
            if let Ok(rd) = fs::read_dir(self.dir()) {
                for e in rd {
                    all.push(format!(
                        "models/{}",
                        e.unwrap().file_name().to_string_lossy()
                    ));
                }
            }
            all.sort();
            all
        }
    }

    /// Known answers (FIPS 180-2): the empty input and "abc".
    #[test]
    fn sha256_hex_is_sha_256() {
        assert_eq!(
            sha256_hex(&b""[..]).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(&b"abc"[..]).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_download_succeeds_and_is_checked_at_load() {
        let s = Setup::new(300_000);
        let server = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        let (result, progress) = s.run(
            &server.url,
            s.expected(),
            LOCAL,
            &plenty,
            &AtomicBool::new(false),
        );
        let path = result.unwrap();
        assert_eq!(fs::read(&path).unwrap(), s.bytes);
        assert_eq!(progress.last(), Some(&Progress::Checking));
        assert_eq!(
            progress[progress.len() - 2],
            Progress::Bytes {
                done: 300_000,
                total: 300_000
            }
        );
        assert_eq!(
            s.files(),
            vec![
                "models",
                "models/ggml-test.bin",
                "models/ggml-test.bin.checked"
            ]
        );
        assert_eq!(check_model(&s.dir(), s.expected()), ModelState::Ready(path));
    }

    #[test]
    fn a_changed_file_is_hashed_again_at_load() {
        let s = Setup::new(200_000);
        let server = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        let path = s.simple(&server.url).unwrap();
        // Same size, other bytes, new modification time: damaged.
        let mut other = s.bytes.clone();
        other[1000] ^= 0xff;
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&path, &other).unwrap();
        assert_eq!(check_model(&s.dir(), s.expected()), ModelState::Damaged);
        // Rewritten with the right bytes (new time): hashed, then ready,
        // and the record renewed.
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&path, &s.bytes).unwrap();
        assert_eq!(
            check_model(&s.dir(), s.expected()),
            ModelState::Ready(path.clone())
        );
        let record = fs::read_to_string(record_path(&s.dir(), "ggml-test.bin")).unwrap();
        assert_eq!(record, stamp(&path).unwrap());
        // Missing.
        delete_model(&s.dir(), "ggml-test.bin").unwrap();
        assert_eq!(check_model(&s.dir(), s.expected()), ModelState::Missing);
        assert_eq!(s.files(), vec!["models"]);
    }

    #[test]
    fn a_wrong_hash_deletes_the_part() {
        let s = Setup::new(100_000);
        let server = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        let wrong = "0".repeat(64);
        let expected = Expected {
            sha256: &wrong,
            ..s.expected()
        };
        let (result, _) = s.run(
            &server.url,
            expected,
            LOCAL,
            &plenty,
            &AtomicBool::new(false),
        );
        assert_eq!(result, Err(DownloadError::Mismatch));
        assert_eq!(s.files(), vec!["models"]);
    }

    #[test]
    fn a_wrong_size_is_refused() {
        let s = Setup::new(100_000);
        let server = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        // Expected smaller than served: refused from Content-Length.
        let small = Expected {
            size: 99_999,
            ..s.expected()
        };
        let (result, _) = s.run(&server.url, small, LOCAL, &plenty, &AtomicBool::new(false));
        assert_eq!(result, Err(DownloadError::Mismatch));
        assert_eq!(s.files(), vec!["models"]);
    }

    #[test]
    fn a_cut_connection_resumes_with_range_after_rehashing_the_part() {
        let s = Setup::new(400_000);
        let server = Server::start(Behaviour::CutOnce(Arc::new(s.bytes.clone()), 150_000));
        let first = s.simple(&server.url);
        assert_eq!(first, Err(DownloadError::Network));
        let part = part_path(&s.dir(), "ggml-test.bin");
        let kept = fs::metadata(&part).unwrap().len();
        assert!(kept > 0 && kept <= 150_000, "{kept}");
        let path = s.simple(&server.url).unwrap();
        assert_eq!(fs::read(path).unwrap(), s.bytes);
        assert_eq!(
            server.ranges(),
            vec![String::new(), format!("bytes={kept}-")]
        );
        assert!(!part.exists());
    }

    #[test]
    fn a_damaged_part_is_caught_by_the_final_hash() {
        let s = Setup::new(200_000);
        let server = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        fs::create_dir_all(s.dir()).unwrap();
        let mut bad = s.bytes[..50_000].to_vec();
        bad[10] ^= 1;
        fs::write(part_path(&s.dir(), "ggml-test.bin"), bad).unwrap();
        assert_eq!(s.simple(&server.url), Err(DownloadError::Mismatch));
        assert_eq!(s.files(), vec!["models"]);
        // The next attempt starts clean and succeeds.
        assert!(s.simple(&server.url).is_ok());
    }

    #[test]
    fn a_redirect_to_another_local_port_is_followed() {
        let s = Setup::new(150_000);
        let target = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        let front = Server::start(Behaviour::Redirect(target.url.clone()));
        let path = s.simple(&front.url).unwrap();
        assert_eq!(fs::read(path).unwrap(), s.bytes);
        assert_eq!(front.ranges().len(), 1);
        assert_eq!(target.ranges().len(), 1);
    }

    #[test]
    fn a_redirect_to_plain_http_is_refused_under_the_release_rule() {
        let s = Setup::new(10_000);
        let target = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        let front = Server::start(Behaviour::Redirect(target.url.clone()));
        // The test server itself is plain HTTP; its redirects follow the
        // app's rule.
        let policy = Policy {
            start: Schemes::HttpsOrLoopback,
            redirects: Policy::RELEASE.redirects,
        };
        let (result, _) = s.run(
            &front.url,
            s.expected(),
            policy,
            &plenty,
            &AtomicBool::new(false),
        );
        assert_eq!(result, Err(DownloadError::Insecure));
        assert!(target.ranges().is_empty(), "never contacted");
        // And the release rule refuses a plain HTTP start before any
        // connection.
        let (result, _) = s.run(
            &front.url,
            s.expected(),
            Policy::RELEASE,
            &plenty,
            &AtomicBool::new(false),
        );
        assert_eq!(result, Err(DownloadError::Insecure));
        assert_eq!(front.ranges().len(), 1);
        assert_eq!(s.files(), vec!["models"]);
    }

    #[test]
    fn not_enough_space_is_refused_before_any_request() {
        let s = Setup::new(10_000);
        let server = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        let free = |_: &Path| Ok(FREE_SPACE_MARGIN + 9_999);
        let (result, _) = s.run(
            &server.url,
            s.expected(),
            LOCAL,
            &free,
            &AtomicBool::new(false),
        );
        assert_eq!(
            result,
            Err(DownloadError::NoSpace {
                needed: FREE_SPACE_MARGIN + 10_000,
                free: FREE_SPACE_MARGIN + 9_999
            })
        );
        assert!(server.ranges().is_empty());
        // Exactly enough is enough.
        let free = |_: &Path| Ok(FREE_SPACE_MARGIN + 10_000);
        let (result, _) = s.run(
            &server.url,
            s.expected(),
            LOCAL,
            &free,
            &AtomicBool::new(false),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn cancel_removes_the_part() {
        let s = Setup::new(400_000);
        let server = Server::start(Behaviour::Slow(Arc::new(s.bytes.clone())));
        let cancel = AtomicBool::new(false);
        let mut progress = |p: Progress| {
            if let Progress::Bytes { done, .. } = p
                && done > 50_000
            {
                cancel.store(true, Ordering::SeqCst);
            }
        };
        let dir = s.dir();
        let result = download(Request {
            url: server.url.clone(),
            expected: s.expected(),
            dir: &dir,
            policy: LOCAL,
            free_space: &plenty,
            cancel: &cancel,
            progress: &mut progress,
        });
        assert_eq!(result, Err(DownloadError::Cancelled));
        assert_eq!(s.files(), vec!["models"]);
    }

    #[test]
    fn two_downloads_into_one_folder_are_refused() {
        let s = Setup::new(10_000);
        let dir = s.dir();
        let _first = Running::claim(&dir).unwrap();
        let server = Server::start(Behaviour::Serve(Arc::new(s.bytes.clone())));
        assert_eq!(s.simple(&server.url), Err(DownloadError::Busy));
        drop(_first);
        assert!(s.simple(&server.url).is_ok());
    }

    #[test]
    fn urls_and_redirect_targets() {
        use Schemes::*;
        assert!(allowed("https://huggingface.co/x", HttpsOnly).is_ok());
        assert!(allowed("HTTPS://cdn.example/x", HttpsOnly).is_ok());
        assert!(allowed("http://cdn.example/x", HttpsOnly).is_err());
        assert!(allowed("http://127.0.0.1:8080/x", HttpsOnly).is_err());
        assert!(allowed("http://127.0.0.1:8080/x", HttpsOrLoopback).is_ok());
        assert!(allowed("http://[::1]:80/x", HttpsOrLoopback).is_ok());
        assert!(allowed("http://localhost/x", HttpsOrLoopback).is_ok());
        assert!(allowed("http://cdn.example/x", HttpsOrLoopback).is_err());
        assert!(allowed("http://127.0.0.1.evil/x", HttpsOrLoopback).is_err());
        assert!(allowed("https://user@host/x", HttpsOnly).is_err());
        assert!(allowed("ftp://host/x", HttpsOrLoopback).is_err());
        assert!(allowed("file:///etc/passwd", HttpsOrLoopback).is_err());
        assert_eq!(
            resolve("https://huggingface.co/a/b", "https://cdn.example/c?x=1").as_deref(),
            Some("https://cdn.example/c?x=1")
        );
        assert_eq!(
            resolve("https://huggingface.co/a/b", "/c/d").as_deref(),
            Some("https://huggingface.co/c/d")
        );
        assert_eq!(resolve("https://huggingface.co/a/b", "//evil/x"), None);
        assert_eq!(resolve("https://huggingface.co/a/b", "relative"), None);
    }

    #[test]
    fn free_space_reads_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_space(dir.path()).unwrap() > 0);
    }
}
