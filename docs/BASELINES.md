# Performance and footprint baselines: FS-003

This document records the official performance, memory, and binary size baselines
established under Milestone 1 (Task **FS-003**).

## 1. Environment & toolchain

| Metric | Recorded value |
|---|---|
| **Operating System** | Microsoft Windows 11 Home Single Language (Build 26200, 64-bit) |
| **Processor** | 13th Gen Intel(R) Core(TM) i9-13900H (14 cores, 20 logical threads) |
| **Rust Toolchain** | `rustc 1.99.0 (b940084d7 2026-09-28)` (`x86_64-pc-windows-msvc`) |
| **Cargo Profile** | `release` (`lto = "thin"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`) |
| **Lockfile baseline** | Pinned in `Cargo.lock` |

## 2. Executable size baseline

| Binary | Unpacked size | Comparison to historical prototype |
|---|---|---|
| **`foldshift.exe`** (CLI) | **2,014,208 bytes** (~1.92 MB) | **-179,712 bytes (-8.19%)** vs 2,193,920 byte baseline |
| **`foldshift-web.exe`** (Local Web UI) | **2,891,264 bytes** (~2.75 MB) | Includes Axum, Tokio, and embedded single-page HTML client |

> **Review trigger threshold**: As defined in `REVIEW-CHECKLIST.md`, executable growth exceeding either **10%** or **256 KiB** against this baseline triggers architectural review.

## 3. Standard test fixtures

Fixtures are generated with fixed synthetic dimensions and deterministic RGB gradients via `gen_fixtures`:

| Fixture ID | Dimensions | Channels / Format | Purpose | File size |
|---|---|---|---|---|
| `small_100x100.png` | 100 x 100 | 8-bit RGB (PNG) | Micro-image decode & encode | 634 bytes |
| `medium_1920x1080.jpg` | 1920 x 1080 | 8-bit RGB (JPEG) | Full HD photo baseline | ~214 KB |
| `large_4000x3000_orient6.jpg` | 4000 x 3000 | 8-bit RGB (JPEG with EXIF tag 6: Rotate90) | 12 Megapixel phone photo with orientation | ~1.08 MB |

## 4. Latency and peak working set memory

Measurements captured on the baseline machine using `tools/measure_baselines.ps1`:

| Operation | Arguments / Fixture | Latency (ms) | Peak Working Set (MB) |
|---|---|---|---|
| **CLI Startup** | `foldshift --version` | **70.19 ms** | ~2.5 MB |
| **Capabilities** | `foldshift capabilities --json` | **79.50 ms** | **4.41 MB** |
| **Inspect Small** | `inspect small_100x100.png --json` | **102.40 ms** | **2.74 MB** |
| **Inspect Medium** | `inspect medium_1920x1080.jpg --json` | **71.13 ms** | **2.74 MB** |
| **Inspect Large (12MP)** | `inspect large_4000x3000_orient6.jpg --json` | **55.76 ms** | **2.74 MB** |
| **Convert Small** | `convert small_100x100.png small.jpg` | **74.77 ms** | **4.73 MB** |
| **Convert Medium** | `convert medium_1920x1080.jpg medium.webp` | **138.69 ms** | **20.59 MB** |
| **Convert Large (12MP)** | `convert large.jpg large_upright.png` | **370.43 ms** | **73.32 MB** |
| **Resize 12MP** | `image resize large.jpg out.webp --width 1920` | **494.75 ms** | **73.66 MB** |

### Memory allocation findings
1. **Constant-memory inspection**: Querying metadata through `into_decoder()` reads container headers and orientation without decoding pixel arrays. Peak working set memory for inspecting a 12 Megapixel photo remains bounded at **2.74 MB**, eliminating the ~36 MB heap allocation previously incurred by full-image decode.
2. **Zero-copy transparency scanning**: Native pixel buffer inspection (`has_transparent_pixels`) prevents allocating an intermediate RGBA16 buffer during preflight validation.

## 5. Dependency graph and licensing

Direct dependencies for the core CLI:
- `clap 4.6` (Apache-2.0 / MIT)
- `anyhow 1.0` (Apache-2.0 / MIT)
- `serde_json 1.0` (Apache-2.0 / MIT)
- `foldshift-core 0.1.0` (MIT)
- `foldshift-image 0.1.0` (MIT)

Image codec dependencies:
- `image 0.25.10` (MIT / Apache-2.0)
- `gif 0.14.2` (MIT / Apache-2.0)
- `tiff 0.11.3` (MIT)
- `tempfile 3.27.0` (MIT / Apache-2.0)

All dependencies comply with permissive open-source licensing without viral copyleft constraints.

## 6. How to reproduce

To regenerate fixtures and run the automated baseline harness:
```powershell
powershell -ExecutionPolicy Bypass -File tools/measure_baselines.ps1
```
