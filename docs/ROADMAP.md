# Roadmap

## v0.1 — image foundation

- [x] Shared job and capability model
- [x] Image inspection
- [x] PNG, JPEG, WebP, GIF, BMP, TIFF conversion
- [x] Proportional resize
- [x] JSON output
- [x] Safe overwrite behavior
- [x] CI workflow for Windows, macOS, and Linux (remote runs pending)
- [x] Local Windows tests for conversion, resizing, corrupt input, and overwrites
- [ ] Golden image fixtures and corruption corpus
- [ ] Shell completion and package manifests

## v0.2 — production image workflows

- Batch and directory processing
- Crop, rotate, and explicit metadata controls
- Presets (`lossless`, `high`, `balanced`, `small`)
- Progress events and cancellation
- AVIF capability evaluation
- Job manifests and input/output hashes

## v0.3 — structural PDF tools

- Optional qpdf adapter
- Merge, split, extract, reorder, rotate, encrypt, decrypt, inspect
- Restricted worker process and resource ceilings
- License and engine provenance reporting

## v0.4 — desktop application

- Thin Tauri interface over the core job API
- Drag and drop, batch queue, presets, progress, cancellation
- Clear local-processing and network state indicators

## Later

- Optional FFmpeg media capability
- Signed capability manifests and updater
- Reproducible pipelines
- Stronger platform sandboxes
- Fuzzing and broader malformed-file corpora

## Explicitly out of scope for early releases

Accounts, cloud sync, AI features, a plugin marketplace, mobile apps, file-history
databases, full editors, custom codecs, and custom compression algorithms.
