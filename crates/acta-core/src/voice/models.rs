//! The Whisper models Acta offers and the VAD it ships (ADR 0014, 4). Each
//! is a compile-time constant: file name, size and SHA-256, checked against
//! the Hugging Face tree API by a weekly CI job. The download is refused
//! for anything that does not match them byte for byte.

/// One file of a model repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelFile {
    pub file: &'static str,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256, lowercase hex.
    pub sha256: &'static str,
    /// `owner/name` on Hugging Face.
    pub repo: &'static str,
}

impl ModelFile {
    /// The download URL: Hugging Face's resolver, anonymous, which
    /// redirects to its CDN.
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            self.repo, self.file
        )
    }
}

/// The Whisper models, smallest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ModelId {
    Tiny,
    Base,
    Small,
    LargeTurbo,
}

impl ModelId {
    pub const ALL: [ModelId; 4] = [
        ModelId::Tiny,
        ModelId::Base,
        ModelId::Small,
        ModelId::LargeTurbo,
    ];

    /// Stable key for `preferences.json`.
    pub fn key(self) -> &'static str {
        match self {
            ModelId::Tiny => "tiny",
            ModelId::Base => "base",
            ModelId::Small => "small",
            ModelId::LargeTurbo => "large-turbo",
        }
    }

    pub fn from_key(key: &str) -> Option<ModelId> {
        ModelId::ALL.into_iter().find(|m| m.key() == key)
    }

    pub fn file(self) -> &'static ModelFile {
        match self {
            ModelId::Tiny => &TINY,
            ModelId::Base => &BASE,
            ModelId::Small => &SMALL,
            ModelId::LargeTurbo => &LARGE_TURBO,
        }
    }
}

const WHISPER_REPO: &str = "ggerganov/whisper.cpp";

pub const TINY: ModelFile = ModelFile {
    file: "ggml-tiny-q5_1.bin",
    size: 32_152_673,
    sha256: "818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7",
    repo: WHISPER_REPO,
};

pub const BASE: ModelFile = ModelFile {
    file: "ggml-base-q5_1.bin",
    size: 59_707_625,
    sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
    repo: WHISPER_REPO,
};

pub const SMALL: ModelFile = ModelFile {
    file: "ggml-small-q5_1.bin",
    size: 190_085_487,
    sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
    repo: WHISPER_REPO,
};

pub const LARGE_TURBO: ModelFile = ModelFile {
    file: "ggml-large-v3-turbo-q5_0.bin",
    size: 574_041_195,
    sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
    repo: WHISPER_REPO,
};

/// Silero VAD, shipped inside the app package (not downloaded by Acta).
pub const SILERO_VAD: ModelFile = ModelFile {
    file: "ggml-silero-v6.2.0.bin",
    size: 885_098,
    sha256: "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987",
    repo: "ggml-org/whisper-vad",
};

/// The free space a download must leave on the models' disk.
pub const FREE_SPACE_MARGIN: u64 = 1_000_000_000;

/// The models' folder inside the data directory (ADR 0003): outside the
/// Velopack root, so updates keep it; not in snapshots, exports or backups.
pub fn models_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("models")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ADR 0014 table, typed again: a constant edited by mistake fails
    /// here before the weekly CI job would see it.
    #[test]
    fn the_table_equals_the_adr() {
        let adr = [
            (
                ModelId::Tiny,
                "ggml-tiny-q5_1.bin",
                32_152_673,
                "818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7",
            ),
            (
                ModelId::Base,
                "ggml-base-q5_1.bin",
                59_707_625,
                "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
            ),
            (
                ModelId::Small,
                "ggml-small-q5_1.bin",
                190_085_487,
                "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
            ),
            (
                ModelId::LargeTurbo,
                "ggml-large-v3-turbo-q5_0.bin",
                574_041_195,
                "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
            ),
        ];
        for (id, file, size, sha) in adr {
            let m = id.file();
            assert_eq!((m.file, m.size, m.sha256), (file, size, sha), "{id:?}");
            assert_eq!(
                m.url(),
                format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file}")
            );
            assert_eq!(ModelId::from_key(id.key()), Some(id));
        }
        assert_eq!(SILERO_VAD.file, "ggml-silero-v6.2.0.bin");
        assert_eq!(SILERO_VAD.size, 885_098);
        assert_eq!(
            SILERO_VAD.sha256,
            "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987"
        );
        assert_eq!(
            SILERO_VAD.url(),
            "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin"
        );
    }

    #[test]
    fn hashes_are_lowercase_hex_of_32_bytes() {
        for m in ModelId::ALL
            .map(ModelId::file)
            .into_iter()
            .chain([&SILERO_VAD])
        {
            assert_eq!(m.sha256.len(), 64);
            assert!(
                m.sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
        }
    }
}
