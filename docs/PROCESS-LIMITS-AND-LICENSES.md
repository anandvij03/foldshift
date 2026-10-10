# Process limits, failure recovery, and license notices

Status: Completed under task `FS-023` (Milestone 3 review).

This document audits and specifies the execution boundaries, failure recovery
guarantees, and third-party license terms for FoldShift's optional engine packs
and subprocess operations, in accordance with the Milestone 3 completion gates.

---

## 1. Process limits and resource restrictions

### 1.1 Subprocess execution model (`foldshift-pdf`)

FoldShift delegates structural PDF operations (`inspect`, `merge`, `extract`) to
a standalone engine binary (`qpdf`) via synchronous subprocess execution.

| Resource boundary | Limit | Enforcement mechanism |
|---|---|---|
| **Execution deadline** | 60 seconds | Process timer polling in `foldshift_pdf::run_qpdf`. On timeout, sends `SIGKILL`/`TerminateProcess`, reaps child, and drains reader threads. |
| **Standard input** | Closed (`Stdio::null()`) | Process cannot wait on interactive console prompts or credentials. |
| **Standard output / error** | 10 MB per stream (`MAX_OUTPUT_BYTES`) | Concurrently read in dedicated worker threads with `std::io::Read::take(10 * 1024 * 1024)`. Eliminates OS pipe buffer deadlocks while preventing output-bomb memory exhaustion on the host. |
| **Child concurrency** | 1 per operation | CLI operations execute sequentially. Batch concurrency is reserved for Milestone 4 bounded worker pools. |

### 1.2 Pack archive and decompression limits (`foldshift-core`)

Offline capability engine packs (`.zip` archives) must satisfy strict pre-flight
and in-flight boundaries prior to installation or update:

| Archive boundary | Limit | Enforcement mechanism |
|---|---|---|
| **Archive file size** | 500 MB (`MAX_ARCHIVE_BYTES`) | Pre-flight file size check before opening archive. |
| **Uncompressed expansion** | $\max(2 \times \text{unpacked\_bytes}, 100\text{ MB})$ | Cumulative uncompressed byte tally during entry pre-scan. Protects against decompression bombs / zip bombs. |
| **Path traversal** | Strict relative paths only | Every entry must pass `is_safe_relative_path` and `ZipFile::enclosed_name()`. Rejects `..`, leading slashes (`/`, `\`), Windows drive letters (`C:`), and null bytes. |
| **File inventory** | Exact match | Post-extraction verification confirms every declared inventory file exists with exact byte lengths and matching SHA-256 hashes. |

### 1.3 Platform boundaries and sandbox truth

> [!IMPORTANT]
> **Truth in Advertising:** FoldShift subprocesses execute in the user's standard
> unprivileged security context. FoldShift **does not** claim or advertise
> kernel-level sandboxing on Windows, macOS, or Linux.

- **Windows**: Executes within the launching user's integrity level. No AppContainer or Job Object limits are claimed until explicitly verified in a future hardening milestone.
- **macOS**: Runs as standard user process without `sandbox_init(3)` / seatbelt profiles.
- **Linux**: Runs as standard user process without `seccomp-bpf` / Linux namespaces.
- **Per-user storage**: All pack installations and state files write exclusively to the user's platform application data directory (`FOLDSHIFT_PACKS_DIR`, with fallbacks to `%LOCALAPPDATA%\foldshift\packs`, `~/Library/Application Support/foldshift/packs`, or `$XDG_DATA_HOME/foldshift/packs`). Neither installation nor execution ever requires elevated/administrator privileges.

---

## 2. Failure recovery and state integrity

FoldShift guarantees that unexpected process termination, power loss, crashes, or
corrupted inputs leave no partial outputs or corrupt states:

### 2.1 Atomic conversion publication
- **Temporary staging**: Outputs are written to an isolated temporary file (`.foldshift_pdf_*.tmp`) in the destination directory using `tempfile::Builder`.
- **Preflight validation**: Same-file collisions without `--force`, missing inputs, invalid PDF headers (`%PDF-`), and malformed page ranges fail before spawning the engine.
- **Crash safety**: If `qpdf` crashes or exits non-zero, the temporary file is unlinked via RAII drop. Existing files and input documents remain untouched.
- **Atomic replacement**: Only after the engine exits with code 0 and disk flush succeeds is `temp_file.persist(output)` called to atomically swap the new file into place.

### 2.2 Atomic pack installation and rollback
- **Isolated staging**: Packs are extracted into `<packs_dir>/.staging/<pack_id>_<version>_<pid>`.
- **Pre-flight rejection**: Incompatible target OS/architecture, incompatible core versions, and checksum mismatches are rejected before touching staging or the destination.
- **Rollback on failure**: If any payload file fails extraction or inventory hash checks, the staging directory is purged immediately. Any existing installed pack in `<packs_dir>/<capability>/<version>` and `installed.json` remains active and untouched.
- **Atomic activation**: Upon 100% extraction and health check success, staging is moved to the versioned directory, and `installed.json` is atomically replaced via temporary file write and rename.
- **Clean removal**: `remove_pack` deletes only the pack's dedicated version directory and purges the parent capability folder if empty. User files and unmanaged dependencies are never touched.

---

## 3. Third-party license notices and governance

> [!NOTE]
> Subprocess separation provides crash isolation, but **does not** eliminate
> copyright or license obligations. Distributing and bundling optional engines
> requires full adherence to upstream licenses.

### 3.1 FoldShift core and image profile
- **License**: MIT License ([LICENSE](../LICENSE)).
- **Codecs**: Pure-Rust image encoders/decoders (`image`, `png`, `gif`, `tiff`).

### 3.2 QPDF engine pack
- **License**: Apache License, Version 2.0.
- **Upstream**: QPDF Project (Jay Berkenbilt and contributors, <https://qpdf.sourceforge.io/>).
- **Notices**: All distributed QPDF pack archives must bundle:
  1. Complete Apache 2.0 license text (`share/LICENSE` or `share/qpdf-license.txt`).
  2. Upstream notice terms (`share/NOTICE`).
- **Manifest visibility**: Every pack manifest specifies `license: "Apache-2.0"` and optional `notice` strings, inspectable by users via `foldshift packs list` and `inspect_pack_archive`.

### 3.3 Future media engine (Milestone 4 preview)
- **License**: FFmpeg (LGPL v2.1+ or GPL v2+ depending on build configuration and enabled external libraries).
- Prior to distributing media packs, an audited build recipe must document all enabled demuxers, decoders, and patent-encumbered features, ensuring clear notice distribution and source availability compliance.
