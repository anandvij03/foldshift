# Foldshift roadmap

Planning baseline: commit `64f0b56`, inspected 4 October 2026.
These are work packages, not promises of release dates or completed features.

## Direction and current state

Process files locally, keep the install small, and let users choose capabilities
at download time. Reuse established libraries and share behavior between CLI and
the eventual UI. Capabilities means images, PDF, audio and video; it does not
mean operating-system filesystems such as NTFS or ext4.

Implemented: still-image conversion, resize, inspection, JSON output, overwrite
protection and temporary-file publication. Images are mandatory today. Capability
reporting is hard-coded. Core-only downloads and pack installation do not exist.
`Job` types exist but no general executor is implemented; the CLI calls image
functions directly. Earlier Windows builds passed nine tests. CI is configured
for three operating systems; verify remote results on each actual change.

Known gaps include silent first-frame/page processing, orientation/profile loss,
whole-image inspection decoding, and no project-defined resource budget or
isolated workers.

## Ownership and ordering

Antigravity implements scoped features. Codex reviews, simplifies and performs
agreed cleanup after handoff. The maintainer decides scope, merges and publishes.
Every implementation task gets a Codex review. These are planning assignments;
no external agent has been started by this document.

| Milestone | ID | Work | Implementation | Depends on | State |
|---|---|---|---|---|---|
| M1: reliable images | FS-001 | Guard animation/page loss; validate requests early | Antigravity | None | Merged |
| M1 | FS-002 | Explicit orientation, color, transparency and quality rules | Antigravity | FS-001 review | Review |
| M1 | FS-003 | Reduce inspection allocations; measure size and memory | Codex cleanup | FS-001 handoff | Queued |
| M1 | FS-004 | Consolidate metadata; remove/defer unused abstraction | Codex cleanup | FS-002/003 | Queued |
| M2: selectable downloads | FS-010 | Optional image feature and accurate capability reporting | Antigravity | M1 | Queued |
| M2 | FS-011 | Core/Images artifacts, download chooser, size manifest | Antigravity | FS-010 | Queued |
| M2 | FS-012 | Verify minimal dependencies, both profiles and packages | Codex review | FS-011 | Queued |
| M3: optional PDF | FS-020 | Versioned engine/pack contract and install state | Antigravity | M2 | Queued |
| M3 | FS-021 | qpdf adapter: inspect, merge, extract pages | Antigravity | FS-020 | Queued |
| M3 | FS-022 | Verified pack install/update/remove and offline import | Antigravity | FS-020 | Queued |
| M3 | FS-023 | Review process limits, recovery and license notices | Codex review | FS-021/022 | Queued |
| M4: batch/media | FS-030 | Sequential batches, names, progress and cancellation | Antigravity | M2 | Queued |
| M4 | FS-031 | Audio adapter and shared media engine pack | Antigravity | M3 | Queued |
| M4 | FS-032 | Video operations and measured concurrency settings | Antigravity | FS-030/031 | Queued |
| M5: desktop | FS-040 | Thin UI over tested operations and capability selection | Antigravity | M2 + stable operation contract | Queued |
| M5 | FS-041 | Review size, startup, memory, accessibility and offline use | Codex review | FS-040 | Queued |

## Completion gates

### M1

- Unsupported animation/page loss requires an explicit decision or fails clearly.
- Orientation, color, alpha, bit-depth and quality behavior are documented/tested.
- Invalid requests fail before full decode or output creation where feasible.
- Inspection uses header information where it supplies the requested data.
- Same-file policy, overwrite races, and failure cleanup are tested.
- Current size, dependency graph, latency and peak memory baselines are recorded
  with machine/toolchain/fixtures; outstanding review findings are tracked.

### M2

- Core and Images build/test separately. Core excludes image codecs.
- Availability matches the real build; missing capabilities never auto-download.
- Every artifact states platform, version, capabilities, download/installed size,
  checksum and notices. Users do not need Rust to run it.
- Selection presents only supported, tested artifacts; custom combinations are
  limited to combinations actually distributed.

### M3

- Structural PDF operations use a versioned engine adapter with tested limits.
- Tampered/incompatible packs and unsafe archive paths are rejected; interrupted
  updates preserve the working version. Offline import works.
- Install/remove preserves user files and required shared engines. Conversion
  cannot initiate downloads.
- Resource/process restrictions are documented per platform. Do not advertise
  sandboxing on a platform until its boundary is implemented and verified.

### M4/M5

- Media availability reflects the exact shipped engine build and codecs.
- Audio/video reuse their engine where applicable; size is not double-counted.
- Batches use bounded concurrency, independent failures and cancellation.
- UI/CLI share behavior; the UI shows actual capabilities and optional downloads.
- Startup/idle memory are measured; closing the application leaves no daemon.

## First experiment

1. Give Antigravity [FS-001](ANTIGRAVITY-HANDOFF.md).
2. Bring its branch/commit and test report to this chat.
3. Codex reviews against [the review checklist](REVIEW-CHECKLIST.md).
4. Resolve findings, then perform cleanup after Antigravity stops editing.
5. Verify the combined commit; the maintainer merges it.

Finish this small handoff before assigning several capabilities at once. Follow
[the collaboration workflow](DEVELOPMENT-WORKFLOW.md) and
[the download design](CAPABILITIES-AND-DOWNLOADS.md).

## Deferred

OCR, Office conversion, plugin marketplaces, arbitrary third-party code loading,
accounts, local HTTP services, pipeline languages, and mobile apps are outside
the first milestones. Reconsider them for a demonstrated use case.
