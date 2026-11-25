## Storage layer

The store keeps every record in one encrypted SQLite file (ADR 0003). Alexandre asked for a summary of the trade-offs:

| Option | Encryption | Notes |
|:-------|:----------:|------:|
| SQLCipher | page-level | Vendored OpenSSL;<br>needs a native Perl on Windows |
| `age` files | per file | No queries<br/>No indexes |
| Plain SQLite | none | **Rejected**: plaintext on disk |

Next steps:

1. Pin `rusqlite` with the `bundled-sqlcipher` feature.
2. Add the key derivation (`argon2id`, 64 MiB).
3. Measure the cold open on the CI runners.
