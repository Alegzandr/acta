//! Where installed apps look for updates: the assets of the release GitHub
//! marks as latest, through `https://github.com/<repo>/releases/latest/download/`
//! (ADR 0004). Plain downloads, not the REST API, so no rate limit applies.
//!
//! Velopack's `HttpSource` would do, except that it appends the installed
//! version, the app id and a random per-install id to the feed URL. The feed
//! request is made here without them; packages are downloaded by `HttpSource`,
//! which adds nothing to those URLs.

use std::path::Path;
use std::sync::mpsc::Sender;

use velopack::bundle::Manifest;
use velopack::sources::{HttpSource, UpdateSource};
use velopack::{Error, VelopackAsset, VelopackAssetFeed};

/// The update source of this app.
pub struct LatestRelease {
    base: String,
    packages: HttpSource,
}

impl LatestRelease {
    pub fn new(repo_url: &str) -> LatestRelease {
        let base = latest_download_base(repo_url);
        LatestRelease {
            packages: HttpSource::new(&base),
            base,
        }
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

impl UpdateSource for LatestRelease {
    fn get_release_feed(
        &self,
        channel: &str,
        _app: &Manifest,
        _staged_user_id: &str,
    ) -> Result<VelopackAssetFeed, Error> {
        let json =
            velopack::download::download_url_as_string(&feed_url(&self.base, channel), None)?;
        Ok(serde_json::from_str(&json)?)
    }

    fn download_release_entry(
        &self,
        asset: &VelopackAsset,
        local_file: &Path,
        progress_sender: Option<Sender<i16>>,
    ) -> Result<(), Error> {
        self.packages
            .download_release_entry(asset, local_file, progress_sender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Against the real latest release: GitHub's redirects to the asset host
    /// resolve with Velopack's client (ureq, rustls) and the feed parses.
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
                feed.Assets.iter().any(|a| a.Type == "Full"),
                "{channel}: no full package"
            );
        }
        // A small asset of the same release, through the package path.
        let file = std::env::temp_dir().join(format!("acta-update-source-{}", std::process::id()));
        let asset = VelopackAsset {
            FileName: "install.sh".into(),
            ..Default::default()
        };
        source.download_release_entry(&asset, &file, None).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        let _ = std::fs::remove_file(&file);
        assert!(text.starts_with("#!/bin/sh"));
    }
}
