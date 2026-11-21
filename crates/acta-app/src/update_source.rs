//! Where installed apps look for updates: the assets of the release GitHub
//! marks as latest, through `https://github.com/<repo>/releases/latest/download/`
//! (ADR 0004). Plain downloads, not the REST API, so no rate limit applies.
//!
//! Velopack's `HttpSource` would do, except that it appends the installed
//! version, the app id and a random per-install id to the feed URL. The feed
//! request is made here without them; packages are downloaded by `HttpSource`,
//! which adds nothing to those URLs.
//!
//! Only full packages are offered. The latest feed holds the delta from the
//! release before it and no other, and Velopack applies every delta newer than
//! the installed version to the installed package without checking the delta's
//! base or the patched result. A client further behind would then depend on
//! zstd catching the wrong base. A full package is checked against the feed's
//! SHA-256 and size before it is applied.

use std::path::Path;
use std::sync::mpsc::Sender;

use velopack::bundle::Manifest;
use velopack::sources::{HttpSource, UpdateSource};
use velopack::{Error, HttpOptions, VelopackAsset, VelopackAssetFeed};

/// Bound on the feed request, connection and transfer. The launch budget
/// (update.rs) is shorter; this only keeps a stalled request from living on.
const FEED_TIMEOUT_MS: u64 = 10_000;
/// Bound on one package download (tens of MB), connection and transfer.
const PACKAGE_TIMEOUT_MS: u64 = 15 * 60_000;

/// The update source of this app.
pub struct LatestRelease {
    base: String,
    packages: HttpSource,
}

impl LatestRelease {
    pub fn new(repo_url: &str) -> LatestRelease {
        let base = latest_download_base(repo_url);
        LatestRelease {
            packages: HttpSource::new_with_options(&base, timeout(PACKAGE_TIMEOUT_MS)),
            base,
        }
    }
}

fn timeout(ms: u64) -> HttpOptions {
    HttpOptions {
        TimeoutMilliseconds: ms,
        ..Default::default()
    }
}

/// `<repo>/releases/latest/download/`, which GitHub redirects to the assets
/// of the latest published, non-pre-release release.
pub fn latest_download_base(repo_url: &str) -> String {
    format!(
        "{}/releases/latest/download/",
        repo_url.trim_end_matches('/')
    )
}

/// The feed of a channel, as `vpk upload` names it, with no query string.
pub fn feed_url(base: &str, channel: &str) -> String {
    format!("{base}releases.{channel}.json")
}

/// The feed without its delta packages.
fn full_packages_only(mut feed: VelopackAssetFeed) -> VelopackAssetFeed {
    feed.Assets
        .retain(|asset| !asset.Type.eq_ignore_ascii_case("Delta"));
    feed
}

/// Whether a feed entry names a file of the release itself. `HttpSource`
/// joins the name to the base URL, so anything else could leave it.
fn is_bare_file_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':', '?', '#'])
}

impl UpdateSource for LatestRelease {
    fn get_release_feed(
        &self,
        channel: &str,
        _app: &Manifest,
        _staged_user_id: &str,
    ) -> Result<VelopackAssetFeed, Error> {
        let json = velopack::download::download_url_as_string(
            &feed_url(&self.base, channel),
            Some(&timeout(FEED_TIMEOUT_MS)),
        )?;
        Ok(full_packages_only(serde_json::from_str(&json)?))
    }

    fn download_release_entry(
        &self,
        asset: &VelopackAsset,
        local_file: &Path,
        progress_sender: Option<Sender<i16>>,
    ) -> Result<(), Error> {
        if !is_bare_file_name(&asset.FileName) {
            return Err(Error::Other(
                "the feed names a package that is not a plain file name".into(),
            ));
        }
        self.packages
            .download_release_entry(asset, local_file, progress_sender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(kind: &str, name: &str) -> VelopackAsset {
        VelopackAsset {
            Type: kind.into(),
            FileName: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn feed_url_is_the_latest_release_asset_without_query() {
        let base = latest_download_base(acta_core::REPO_URL);
        assert_eq!(
            base,
            "https://github.com/Alegzandr/acta/releases/latest/download/"
        );
        assert_eq!(
            feed_url(&base, "linux-x64"),
            "https://github.com/Alegzandr/acta/releases/latest/download/releases.linux-x64.json"
        );
        assert_eq!(
            latest_download_base("https://github.com/Alegzandr/acta/"),
            base
        );
    }

    #[test]
    fn deltas_are_dropped_from_the_feed() {
        let feed = VelopackAssetFeed {
            Assets: vec![
                asset("Full", "Acta-0.1.4-linux-x64-full.nupkg"),
                asset("Delta", "Acta-0.1.4-linux-x64-delta.nupkg"),
                asset("delta", "Acta-0.1.4-linux-x64-delta2.nupkg"),
            ],
        };
        let names: Vec<_> = full_packages_only(feed)
            .Assets
            .into_iter()
            .map(|a| a.FileName)
            .collect();
        assert_eq!(names, ["Acta-0.1.4-linux-x64-full.nupkg"]);
    }

    #[test]
    fn only_bare_file_names_are_downloaded() {
        assert!(is_bare_file_name("Acta-0.1.4-win-x64-full.nupkg"));
        for bad in [
            "",
            ".",
            "..",
            "../x.nupkg",
            "a/b.nupkg",
            "a\\b.nupkg",
            "https://example.com/x.nupkg",
            "C:x.nupkg",
            "x.nupkg?y",
            "x.nupkg#y",
        ] {
            assert!(!is_bare_file_name(bad), "{bad:?}");
        }
        let source = LatestRelease::new(acta_core::REPO_URL);
        let file = std::env::temp_dir().join("acta-update-source-never-written");
        let err = source
            .download_release_entry(&asset("Full", "../x.nupkg"), &file, None)
            .unwrap_err();
        assert!(matches!(err, Error::Other(_)));
        assert!(!file.exists());
    }

    /// Against the real latest release: GitHub's redirects to the asset host
    /// resolve with Velopack's client (ureq, rustls), the feed parses and
    /// keeps its full package.
    /// `cargo test -p acta-app -- --ignored latest_release`.
    #[test]
    #[ignore = "network"]
    fn latest_release_feed_and_assets_download() {
        let source = LatestRelease::new(acta_core::REPO_URL);
        let manifest = Manifest::default();
        for channel in ["win-x64", "osx-arm64", "osx-x64", "linux-x64"] {
            let feed = source
                .get_release_feed(channel, &manifest, "")
                .unwrap_or_else(|e| panic!("{channel}: {e}"));
            assert!(
                feed.Assets.iter().all(|a| a.Type == "Full") && feed.Assets.len() == 1,
                "{channel}: expected the full package only"
            );
        }
        // A small asset of the same release, through the package path.
        let file = std::env::temp_dir().join(format!("acta-update-source-{}", std::process::id()));
        source
            .download_release_entry(&asset("", "install.sh"), &file, None)
            .unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        let _ = std::fs::remove_file(&file);
        assert!(text.starts_with("#!/bin/sh"));
    }
}
