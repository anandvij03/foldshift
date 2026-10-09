# Foldshift

Foldshift is a small, local-first file utility. Files are processed on the
user's machine: no account, upload, telemetry, database, or conversion daemon is
required.

The project is intentionally split into a lightweight Rust core and optional
capabilities. Version 0.1 begins with image conversion because it can provide a
useful, testable workflow without bundling large native engines.

**Convert files. Keep them yours.**

Status: early development. The current CLI supports still images. PDF, media,
and a graphical interface are planned. No public release has been published yet.

## Current v0.1 scope

- Inspect images and report format, dimensions, color type, and file size.
- Convert between PNG, JPEG, WebP, GIF, BMP, and TIFF.
- Resize proportionally within a width/height bounding box.
- Detect and reject animated or multi-page inputs (GIF, APNG, WebP, TIFF) before
  conversion or decode to prevent silent content loss.
- Preflight destination format, quality, and collisions early without creating
  files or directories on invalid requests.
- Strip embedded metadata as a natural consequence of decode/re-encode.
- Refuse to overwrite an existing file unless `--force` is supplied.
- Write to a temporary sibling file before moving the completed output into
  place.
- Expose machine-readable JSON inspection and capability output.

PDF, audio/video, AVIF/HEIC, the desktop UI, engine installation, sandboxing,
and self-updates are roadmap items, not current features.

## Build

Install Rust 1.88 or newer (current stable recommended), then run:

```bash
cargo build --release --locked
```

The executable is `target/release/foldshift` (`foldshift.exe` on Windows). In PowerShell,
invoke it as `.\foldshift.exe` from the folder containing the executable.

## Usage

```bash
foldshift capabilities
foldshift inspect photo.png
foldshift inspect photo.png --json

foldshift convert photo.png photo.webp
foldshift convert photo.png photo.jpg --quality 88

foldshift image resize photo.jpg resized.webp --width 1600 --height 1600
foldshift image resize photo.jpg resized.jpg --width 1600 --quality 85 --force
```

At least one of `--width` and `--height` is required for resize. When only one
is given, the other dimension is inferred. When both are given, the original
aspect ratio is preserved and the result fits inside the requested box.

The current image path processes still images. Animated inputs (GIF, APNG, WebP)
and multi-page TIFFs are rejected with an explicit error to prevent silent
content loss. Single-frame GIF and single-page TIFF are supported. Embedded color
profiles and orientation metadata are not yet preserved or applied. `--quality`
controls JPEG only; WebP output is lossless. JPEG export rejects transparent
pixels; PNG and WebP preserve transparency.

Temporary files are managed by the `tempfile` library. Replacement uses its
persist operation; without `--force`, finalization refuses an existing target,
including a target created while encoding was running. Invalid requests fail
before full decode and do not create output directories or temporary files.

## Architecture

```text
CLI / future desktop UI
          |
       foldshift-core       shared jobs, formats, errors, capability registry
          |
       foldshift-image      pure Rust image capability
          |
   optional adapters (future qpdf / FFmpeg workers)
```

The UI will only submit jobs to the same core used by the CLI. It will not own a
second conversion implementation.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
[docs/ROADMAP.md](docs/ROADMAP.md), and [SECURITY.md](SECURITY.md).

## Privacy model

The v0.1 crates have no networking dependency or update code. Conversion reads
the requested input and writes the requested output locally. Future download and
update functionality must live outside the conversion core.

## License

MIT. Optional capability packs may include separately licensed engines; their
licenses must be surfaced before installation and in provenance output.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) for setup, tests, and contribution
guidelines. Start with a small issue from the [roadmap](docs/ROADMAP.md).
Bug reports should use synthetic or redacted files.

Maintainers: follow the [GitHub launch checklist](docs/GITHUB-SETUP.md) before
publishing and the [release checklist](docs/RELEASING.md) for each release.
