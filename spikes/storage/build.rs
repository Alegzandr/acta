//! Only used by the `ltc` feature: compiles SQLCipher with its libtomcrypt provider from the
//! `sqlcipher-src` crate, so the build needs a C compiler and nothing else (no OpenSSL, no Perl).
//! `libsqlite3-sys` (feature `sqlcipher`, not bundled) then links the `sqlcipher` library built here.

fn main() {
    #[cfg(feature = "ltc")]
    ltc::build();
}

#[cfg(feature = "ltc")]
mod ltc {
    // One libtomcrypt configuration shared by every unit: only what SQLCipher's provider uses.
    const LTC: &[&str] = &[
        "LTC_NOTHING",
        "LTC_RIJNDAEL",
        "LTC_CBC_MODE",
        "LTC_SHA1",
        "LTC_SHA256",
        "LTC_SHA512",
        "LTC_HASH_HELPERS",
        "LTC_HMAC",
        "LTC_FORTUNA",
        "LTC_PKCS_5",
        "LTC_RNG_GET_BYTES",
        "LTC_PRNG_ENABLE_LTC_RNG",
        "LTC_NO_TEST",
        "LTC_NO_FILE",
        "LTC_NO_ASM",
        "LTC_NO_MATH",
        "LTC_CLEAN_STACK",
    ];
    // Same SQLite options as libsqlite3-sys's own bundled build.
    const SQLITE: &[(&str, Option<&str>)] = &[
        ("SQLITE_CORE", None),
        ("SQLITE_DEFAULT_FOREIGN_KEYS", Some("1")),
        ("SQLITE_ENABLE_API_ARMOR", None),
        ("SQLITE_ENABLE_COLUMN_METADATA", None),
        ("SQLITE_ENABLE_DBSTAT_VTAB", None),
        ("SQLITE_ENABLE_FTS5", None),
        ("SQLITE_ENABLE_JSON1", None),
        ("SQLITE_ENABLE_STAT4", None),
        ("SQLITE_THREADSAFE", Some("1")),
        ("SQLITE_USE_URI", None),
        ("HAVE_USLEEP", Some("1")),
        ("HAVE_ISNAN", None),
        ("HAVE_STDINT_H", Some("1")),
        ("SQLITE_HAS_CODEC", None),
        ("SQLITE_TEMP_STORE", Some("2")),
        ("SQLITE_EXTRA_INIT", Some("sqlcipher_extra_init")),
        ("SQLITE_EXTRA_SHUTDOWN", Some("sqlcipher_extra_shutdown")),
        ("SQLCIPHER_CRYPTO_LIBTOMCRYPT", None),
    ];

    pub fn build() {
        let dir = sqlcipher_src::source_dir();
        let mut cc = cc::Build::new();
        cc.warnings(false)
            .include(dir.join(sqlcipher_src::LIBTOMCRYPT_INCLUDE_DIR));
        for d in LTC {
            cc.define(d, None);
        }
        let mut ltc = cc.clone();
        ltc.define("LTC_SOURCE", None);
        ltc.files(
            sqlcipher_src::LIBTOMCRYPT_SOURCES
                .iter()
                .map(|s| dir.join(s)),
        );
        ltc.compile("sqlcipher_ltc");

        for (k, v) in SQLITE {
            cc.define(k, *v);
        }
        if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
            cc.define("HAVE_LOCALTIME_R", None);
        } else {
            println!("cargo:rustc-link-lib=advapi32");
        }
        cc.file(dir.join(sqlcipher_src::SOURCE_FILE));
        cc.compile("sqlcipher");
    }
}
