# Codex review and lightweight cleanup

This is a review queue, not a completed security audit. Observations come from
commit `64f0b56` inspected 4 October 2026; recheck against the submitted diff.

## Initial queue

| Area | Observed implementation | Action |
|---|---|---|
| Inspection memory | `inspect()` fully decodes for dimensions/color | FS-003: header/decoder metadata where supported; measure allocation reduction |
| JPEG allocation | Full RGBA16 copy for alpha check, then RGB8 copy | FS-003: inspect native alpha where possible, retaining bit-depth correctness |
| Validation | Conversion decodes before output/quality checks | FS-001: preflight plus final no-clobber publication check |
| Frame/page loss | One `DynamicImage` passes through conversion | FS-001: explicit guard for unsupported multi-frame/page content |
| Availability | Registry and CLI independently hard-code it | FS-010: derive from compiled/installed capabilities |
| Minimal builds | CLI always depends on images | FS-010/012: optional dependency and targeted graph verification |
| Abstractions | Generic `Job`/`Format` types have no executor | FS-004: justify current consumers, simplify or defer |
| Errors | `is_file() == false` always reports missing input | Distinguish directory, permission and other I/O errors |
| Same-file behavior | `--force` can target the input path | Specify alias/same-file policy and test preservation |

Do not combine this whole queue into the first PR. Measure each optimization
against consistent fixtures.

## Before review

- [ ] Identify base/review commit, accepted task and changed files.
- [ ] Confirm the implementation agent stopped before cleanup in that checkout.
- [ ] Read instructions, preserve unrelated work and check for expanded scope.

## Correctness

- [ ] Preserve original files on failures, cancellation and rejected requests.
- [ ] Validate options and inspect content rather than trusting file extensions.
- [ ] Check dimensions, frames/pages, alpha, orientation, profiles and bit depth.
- [ ] Test overwrite races, force, same-file aliases and publication failures.
- [ ] Avoid visible partial outputs; handle flush/persist errors and cleanup.
- [ ] Set/test resource bounds and describe platform enforcement limits.
- [ ] Independently reopen outputs for format-related changes.

## Simplicity and footprint

- [ ] Reuse library APIs and avoid abstractions with no current consumer.
- [ ] Justify dependencies and enabled features; inspect the actual graph.
- [ ] Keep installer networking out of conversion.
- [ ] Record compressed download bytes and unpacked executable/pack bytes.
- [ ] Measure startup, inspect/convert latency and peak memory with fixed fixtures.

An earlier Windows prototype measured 2,193,920 executable bytes. This historical
number is not a current per-platform guarantee. FS-003 establishes a reproducible
baseline with OS, architecture, toolchain, profile, lockfile and fixture IDs.

Proposed initial review trigger: explain executable growth exceeding either
10% or 256 KiB against a comparable baseline. This triggers review, not automatic
rejection. A useful optional engine can be large if users see its measured cost
and installations without that capability exclude it.

## Review result

Give actionable findings with file/line, trigger, consequence and suggested
direction. Separate blockers from optional cleanup. Record the exact revision,
checks/results, untested platforms and outstanding limitations. After cleanup,
review the combined diff and repeat relevant checks. The maintainer merges and
publishes.

---

## Milestone 3 Review & Audit (FS-023)

Audit date: 10 October 2026  
Reviewed commits: `92c5ddb` (FS-020), `ee0b66c` (FS-021), `522791f` (FS-022)  

### Findings and Dispositions

1. **Subprocess Pipe Buffer Deadlock Risk in `run_qpdf`**:
   - **Trigger**: Child process generating stdout or stderr exceeding OS pipe buffer limits (4KB–64KB) while parent process polls `child.try_wait()`.
   - **Consequence**: Child blocks on `write()`, parent waits in loop until 60s timeout, causing hang and false failure.
   - **Remediation**: Replaced sequential wait with dual concurrent worker threads draining `stdout` and `stderr` streams in the background as the process writes. Added 10 MB per-stream safety cap (`MAX_OUTPUT_BYTES`) to prevent memory exhaustion from output bombs.

2. **Atomic Recovery and Interrupted Update Semantics**:
   - **Verified**: Staging directory isolation (`.staging`) ensures broken or aborted pack installations and updates never touch existing destination directories or active records in `installed.json`.
   - **Verified**: Atomic `persist()` in `foldshift-pdf` guarantees original input files and destinations are never truncated or modified if `qpdf` exits with an error or is terminated.

3. **Platform Sandbox Boundary Truth**:
   - **Audit**: Confirmed that FoldShift does not advertise kernel-level sandboxing on any platform.
   - **Documentation**: Documented platform execution contexts and boundaries in `docs/PROCESS-LIMITS-AND-LICENSES.md`.

4. **Third-Party License Governance**:
   - **Audit**: Verified that QPDF is governed by the Apache 2.0 license, and that pack manifests mandate `license` and `notice` tracking.
   - **Documentation**: Recorded license obligations in `docs/PROCESS-LIMITS-AND-LICENSES.md`.

---

## Milestone 4 Review & Audit (FS-030)

Audit date: 24 October 2026  
Reviewed commits: `72e4186` (FS-030 core batch), `41c8612` (web & installers)  

### Findings and Dispositions

1. **Batch Isolation & Independent Failures**:
   - **Verified**: Batch processing loops execute items sequentially. If item $i$ encounters an error (corrupted payload, unsupported format, or collision without force), the error is isolated to that item and execution proceeds without halting the batch.
   - **Verified**: Atomic publication via `tempfile` ensures in-progress failures never leave corrupt or partial outputs in destination directories.

2. **Naming Templates and Path Safety**:
   - **Verified**: Placeholders `{stem}`, `{name}`, `{ext}`, `{index}`, and zero-padded indices resolve strictly to base filenames.
   - **Verified**: Directory traversal (`..`) and path separators (`/`, `\`) in pattern resolutions are rejected with `FcError::InvalidOption`.
   - **Verified**: Intra-batch collisions (where multiple input files map to the same output path) are caught early during batch planning.

3. **Cooperative Cancellation & Signal Handling**:
   - **Verified**: `CancellationToken` enables clean cancellation. CLI intercepts `SIGINT` (Ctrl+C) via `ctrlc`, safely completing active atomic writes and marking remaining items as cancelled without data corruption.
   - **Verified**: A second interrupt triggers an immediate emergency exit (`std::process::exit(130)`).

4. **Dependency Minimality**:
   - **Verified**: Core-only profile has 0 image codecs. `ctrlc` added solely to CLI for cross-platform signal management without impacting core libraries.


