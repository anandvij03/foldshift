<div align="center">

# ⚡ FoldShift

**Convert files. Keep them yours.**

*An ultra-lightweight (<3 MB), 100% offline, privacy-first file conversion and processing engine written in Rust.*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.88%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](#quick-install)
[![Binary Size](https://img.shields.io/badge/Binary%20Size-%3C3%20MB-success.svg)](#selectable-downloads)
[![Privacy](https://img.shields.io/badge/Privacy-100%25%20Offline%20%7C%20Zero%20Telemetry-blueviolet.svg)](#why-foldshift)

[Download Binaries](https://github.com/anandvij03/foldshift/releases) • [Quickstart](#quickstart) • [Why FoldShift](#why-foldshift) • [Documentation](docs/) • [Roadmap](docs/ROADMAP.md)

</div>

---

Stop uploading sensitive financial records, private photos, and legal documents to third-party online converters. **FoldShift** runs entirely on your local machine with zero background daemons, zero accounts, and zero cloud telemetry.

It ships as a **single, self-contained binary** featuring both a blazing-fast command-line interface and an embedded zero-dependency local drag-and-drop web UI.

---

## 🚀 Quick Install

No Rust toolchain, C runtimes, or package managers required. Choose your platform:

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/anandvij03/foldshift/main/tools/install.ps1 | iex
```

### macOS & Linux (Shell)
```bash
curl -fsSL https://raw.githubusercontent.com/anandvij03/foldshift/main/tools/install.sh | sh
```

### Direct Download
Grab pre-compiled static standalone binaries directly from [**GitHub Releases**](https://github.com/anandvij03/foldshift/releases).

### From Source (Rust developers)
```bash
# Image CLI
cargo install foldshift-cli --features image

# Local Web UI
cargo install foldshift-web
```

---

## 💡 Why FoldShift?

| Feature | FoldShift | Online Converters *(CloudConvert, iLovePDF)* | Traditional CLI *(ImageMagick, FFmpeg)* | Desktop GUI Apps *(Electron)* |
|---|:---:|:---:|:---:|:---:|
| **Privacy & Security** | 🔒 **100% Offline** (Zero uploads) | ❌ Files processed on 3rd-party servers | 🔒 100% Offline | ⚠️ Often telemetry-heavy |
| **Binary Size** | ⚡ **~1.9 – 2.8 MB** | N/A (Browser SaaS) | ⚠️ 50 – 200+ MB | ❌ 150 – 400+ MB |
| **User Interface** | 🖥️ **Both CLI & Local Web UI** | 🌐 Cloud Web only | 💻 CLI only | 🖥️ Desktop GUI only |
| **Silent Loss Safeguards** | 🛡️ **Explicit preflights** | ⚠️ Varies | ⚠️ Silent frame/page drops | ⚠️ Varies |
| **Dependencies** | 📦 **Zero runtime dependencies** | Internet required | Heavy C/C++ runtimes | Chromium runtime |

---

## 📦 Selectable Downloads

FoldShift is built modularly so you only download what you need. All binaries run offline without runtime accounts or external frameworks.

| Profile | Included Capabilities | Installed Size | Download Size (ZIP) | SHA-256 Checksum |
|---|---|---|---|---|
| **Foldshift Core** | `core` | **0.45 MB** | **0.21 MB** | `b2e57086e11b532f099ce67e1c77e6505ad9a6bde0ed0bf591f1186c36b773a6` |
| **Foldshift Images** *(Recommended CLI)* | `core`, `image` | **1.92 MB** | **0.86 MB** | `76d4329528a14a3d701ead8b435277701265e2dd7666f6503fcba1b8d0b8064b` |
| **Foldshift Web** *(Recommended for GUI)* | `core`, `image`, `web` | **2.76 MB** | **1.31 MB** | `afb7cc5e0e9180ffea3b2aeaa592488862fb5dd73782b3aa1cef2b01fc06e7f5` |

* Machine-readable metadata and checksums are published in [`dist/manifest.json`](dist/manifest.json).

---

## 🛠️ Quickstart

### 1. Launch the Local Web UI
Double-click `foldshift-web` (or run it from your terminal):
```bash
foldshift-web
# Listening on http://127.0.0.1:3000
```
Open your browser to `http://localhost:3000` for a clean, zero-cloud drag-and-drop file processing suite.

### 2. Command Line Operations

#### Inspect Files
```bash
foldshift inspect photo.png
foldshift inspect document.pdf --json
```

#### Convert Formats
Convert between PNG, JPEG, WebP, GIF, BMP, and TIFF with atomic write safety:
```bash
foldshift convert scan.png output.webp
foldshift convert raw.tiff photo.jpg --quality 90
```

#### Aspect-Preserving Resize
```bash
foldshift resize banner.png small.png --max-width 800 --max-height 600
```

#### Batch Processing
```bash
foldshift batch convert *.png --to webp --naming "{name}_converted.{ext}"
```

#### Modular Packs (PDF Tools)
Inspect and manage external engine adapters like `qpdf`:
```bash
foldshift packs list
foldshift pdf merge doc1.pdf doc2.pdf -o merged.pdf
foldshift pdf extract book.pdf --pages 1-5 -o excerpt.pdf
```

---

## 🔒 Safety Guarantees

- **No Silent Loss:** FoldShift checks file structure upfront. Attempting to convert multi-page TIFFs or animated GIFs into single-frame formats without explicit confirmation fails safely to protect your data.
- **Atomic Operations:** Outputs are written to isolated temporary files and replaced atomically upon successful conversion. A failed conversion will never corrupt an existing file.
- **Strict Overwrite Protection:** Will never overwrite destination files unless `--force` is explicitly provided.

---

## 🗺️ Roadmap & Ecosystem

FoldShift follows an incremental, strictly tested roadmap:
- [x] **M1: Reliable Still Images** (Inspection, conversion, resize, loss guards)
- [x] **M2: Selectable Modular Downloads** (Core vs Images vs Web distribution)
- [x] **M3: Capability Packs & PDF Engine** (`qpdf` adapter, page extraction, merging)
- [x] **M4: Batch Processing & Progress** (Sequential batches, cancellation, naming tokens)
- [ ] **M5: Native Desktop Window** (Thin offline window wrapper over local engine)
- [ ] **M6: Audio & Video Packs** (FFmpeg-backed audio/video extraction & transcoding)

Read the full planning specification in [docs/ROADMAP.md](docs/ROADMAP.md).

---

## 🤝 Contributing

Contributions, bug reports, and suggestions are warmly welcome!
1. Check existing issues or file a new one via [GitHub Issues](https://github.com/anandvij03/foldshift/issues).
2. Follow the [Contributing Guide](CONTRIBUTING.md) and [Development Workflow](docs/DEVELOPMENT-WORKFLOW.md).
3. Ensure all tests and Clippy checks pass before opening a PR:
   ```bash
   cargo clippy --workspace --all-targets --locked
   cargo test --workspace --locked
   ```

---

## 📜 License

FoldShift is licensed under the [MIT License](LICENSE).  
*Convert files. Keep them yours.*
