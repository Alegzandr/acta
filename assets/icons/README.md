# Acta app icon

The mark is a capital A whose crossbar is a ledger rule in the accent colour. The rule touches the left leg, stops short of the right leg, and continues past it off the edge of the plate, like a line leaving the page. The plate is `#42413D`, the A is `#E4E3DF` and the rule is verdigris `#72B5A2` (the dark-theme `accent` of DESIGN.md).

## Files

| File | Used by |
|---|---|
| `acta.svg` | Master for Windows and Linux: a 64-unit grid, full-bleed plate with radius 14. |
| `acta-macos.svg` | macOS master: an 824 px continuous-curvature plate (corner r = 185.4) centred in a 1024 canvas, with no shadow. |
| `acta-symbolic.svg` | Monochrome version: a `currentColor` plate with the A and the rule knocked out. |
| `acta.ico` | Windows executable icon (`crates/acta-app/build.rs`, `scripts/package.ps1`). Sizes 16, 20, 24, 32, 40, 48, 64 and 256; the 256 entry is PNG-compressed. |
| `acta.icns` | macOS bundle icon (`scripts/package.sh`). The full iconset from 16@1x to 512@2x, stored as PNG entries. |
| `acta.png` | 512 x 512. Used for the Linux desktop entry (`desktop_entry.rs`), the Velopack `--icon` and the updater window's app mark (`icons.rs`). |
| `hicolor/` | Freedesktop icon theme tree: 16, 22, 24, 32, 48, 64, 128 and 256 px, plus `scalable/apps/acta.svg` and `symbolic/apps/acta-symbolic.svg`. |

Nothing installs the `hicolor/` tree yet: the AppImage desktop entry installs only the 512 px `acta.png`.

The 16, 20, 22, 24 and 32 px bitmaps are hand-hinted from pixel tables, not downscaled: every glyph pixel is opaque, and the gaps on each side of the right leg are exactly 1 px. All other sizes are renders of the vector masters.

## Provenance and licence

This is original work for Acta. The concept came from explorations made with FLUX.2 Klein 4B running locally. The final geometry was drawn by hand and is written out in `scripts/icon/build_icons.py`; no generated pixels are shipped. The icon is covered by the same terms as the rest of this repository.

## Regenerating

From the repository root, with Python 3.10 or later:

```
python -m venv .venv-icon
.venv-icon/Scripts/python -m pip install -r scripts/icon/requirements.txt   # bin/ instead of Scripts/ outside Windows
.venv-icon/Scripts/python scripts/icon/build_icons.py
```

The script rewrites every file listed above. To change the mark, edit the paths and hint tables in the script, never the generated files.
