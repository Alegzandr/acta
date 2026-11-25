# Release plan for 0.6.0

1. Prepare the branch
   1. Merge `main` into `release/0.6`
   2. Run the full suite:
      - `cargo fmt --all`
      - `cargo clippy --workspace --all-targets -- -D warnings`
      - `cargo test --workspace`
   3. Bump the version
2. Build the installers
   1. Windows (Velopack)
   2. macOS (unsigned `.app`)
   3. Linux (AppImage)

      7. A list nested deeper, starting at 7 after a blank line
      8. Eight
3. Publish

- [x] Changelog written by Alexandre
- [ ] Screenshots updated
  - [x] Light theme
  - [ ] Dark theme
- [ ] Announcement
