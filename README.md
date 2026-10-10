# foldshift
A lightweight, local file processing and editing framework that works modularly and for almost any file type

**Convert files. Keep them yours.**

The current implementation provides a lightweight Rust CLI and a local,
browser-based web UI for image conversion, resizing, and inspection. Broader
format support and a desktop UI are planned.

## Selectable downloads

Choose the capability set that fits your workflow. All binaries run locally and offline with zero background services, telemetry, or runtime accounts. Users do not need Rust installed to run pre-compiled binaries.

### Windows (x86_64)

| Selection | Capabilities | Installed Size | Download Size (ZIP) | SHA-256 Checksum |
|---|---|---|---|---|
| **Foldshift Core** | `core` | **0.45 MB** (473,600 B) | **0.21 MB** (223,818 B) | `b2e57086e11b532f099ce67e1c77e6505ad9a6bde0ed0bf591f1186c36b773a6` |
| **Foldshift Images** | `core`, `image` | **1.92 MB** (2,016,256 B) | **0.86 MB** (900,641 B) | `76d4329528a14a3d701ead8b435277701265e2dd7666f6503fcba1b8d0b8064b` |
| **Foldshift Web** | `core`, `image`, `web` | **2.76 MB** (2,890,752 B) | **1.31 MB** (1,374,819 B) | `afb7cc5e0e9180ffea3b2aeaa592488862fb5dd73782b3aa1cef2b01fc06e7f5` |

- **Core**: Lightweight binary (under 500 KB) for capability discovery, inspection contracts, and CLI management without conversion codecs.
- **Images (Recommended)**: Base CLI plus pure-Rust still-image inspection, conversion, and aspect-preserving resizing (PNG, JPEG, WebP, GIF, BMP, TIFF).
- **Web**: Local HTTP server and embedded zero-dependency browser UI for drag-and-drop operations on `localhost`.

Full machine-readable metadata is available in [`dist/manifest.json`](dist/manifest.json).

- [Project overview and commands](PROJECT-OVERVIEW.md)
- [Contributing](CONTRIBUTING.md)
- [Roadmap](docs/ROADMAP.md)
- [Development workflow and ownership](docs/DEVELOPMENT-WORKFLOW.md)
- [Selectable downloads design](docs/CAPABILITIES-AND-DOWNLOADS.md)
- [Performance and footprint baselines](docs/BASELINES.md)
- [First Antigravity handoff](docs/ANTIGRAVITY-HANDOFF.md)
- [Code review and cleanup checklist](docs/REVIEW-CHECKLIST.md)
- [GitHub setup checklist](docs/GITHUB-SETUP.md)
- [Release checklist](docs/RELEASING.md)

Licensed under [MIT](LICENSE).
