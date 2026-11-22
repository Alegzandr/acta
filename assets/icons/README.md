# Acta app icon

The icon is Companion: an ivory planet with a thin brass ring tilted around it, and a small verdigris moon beyond the ring at the upper right, on a dark ink plate. The ring passes in front of the planet's lower half and behind its upper half, and casts a thin shadow on it. Nothing touches anything else. The verdigris is the dark-theme `accent` of `docs/design/color.md` (`#72B5A2`).

## Files

| File | Used by |
|---|---|
| `acta.ico` | Windows executable icon (`crates/acta-app/build.rs`, `scripts/package.ps1`). Sizes 16, 20, 24, 32, 40, 48, 64 and 256; the 256 entry is PNG-compressed. |
| `acta.icns` | macOS bundle icon (`scripts/package.sh`). The full iconset from 16@1x to 512@2x, as PNG entries, on Apple's 824 px continuous-curvature plate inside a 1024 canvas, with no shadow. |
| `acta.png` | 512 x 512. Used for the Linux desktop entry (`desktop_entry.rs`), the Linux Velopack `--icon` (`scripts/package.sh`) and the updater window's app mark (`icons.rs`). |
| `acta-symbolic.svg` | Monochrome silhouette in `currentColor`: planet, ring (the front arc cut out of the planet, the back arc hidden behind it) and moon. Hand-written. |
| `hicolor/` | Freedesktop icon theme tree: 16, 22, 24, 32, 48, 64, 128 and 256 px, plus `symbolic/apps/acta-symbolic.svg`. The 16, 24 and 32 px bitmaps are the titlebar icon on Windows and Linux (`titlebar.rs`), one per scale factor. |

Nothing installs the `hicolor/` tree yet: the AppImage desktop entry installs only the 512 px `acta.png`.

The bitmaps from 40 px up are reductions of one 1024 render. 16, 20, 22, 24 and 32 px are rendered separately from the same scene with a thicker ring (1 to 1.2 px) and a slightly larger moon, so the ring stays visible and the moon stays clear of it. Windows and Linux use a full-bleed superellipse plate (n = 5).

## Provenance and licence

This is original work for Acta. It is rendered by a script, `scripts/icon/tracer.py`: analytic spheres and a swept tube, Lambert and specular shading, and procedural noise for the moon's mottling and the plate's grain. No generated or third-party pixels are shipped. The direction was chosen from explorations made with FLUX.2 Klein 4B running locally, but none of those images are used. The icon is covered by the same terms as the rest of this repository.

## Regenerating

From the repository root, with Python 3.12 or later (numpy 2.5.5 requires it):

```
python -m venv .venv-icon
.venv-icon/Scripts/python -m pip install -r scripts/icon/requirements.txt   # bin/ instead of Scripts/ outside Windows
.venv-icon/Scripts/python scripts/icon/build_icons.py
```

The output is deterministic on a given platform (verified on Windows x64). The scene (positions, sizes, colours, light) is the `SPEC` table in `tracer.py`, and the per-size adjustments are in `build_icons.py`. Edit those, never the generated files.
