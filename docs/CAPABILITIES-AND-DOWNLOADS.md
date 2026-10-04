# Selectable capabilities and downloads

Status: planned, 4 October 2026. Today images are bundled; there is no installer.
Pack commands and optional-feature build commands below are proposals.

## User experience

Choose OS/architecture, then functions. Show the added download bytes and
installed bytes for each selection. Optional extras are not preselected.
Installation needs no account; subsequent file processing works offline.

| Selection | Functions | Delivery |
|---|---|---|
| Core | Help, version, capability discovery; later management entry points | Base executable without conversion codecs |
| Images | Still-image inspect, convert, resize; later batch tools | Initially compiled into the Images profile |
| PDF | Structural inspect, merge, page extraction | Later qpdf pack; rendering/OCR separate |
| Audio | Supported audio conversion and inspection | Later media pack with audited build configuration |
| Video | Video processing and audio extraction | Reuse the media engine where supported |
| Desktop interface | File selection, jobs and capability controls | Separate optional distribution |

Only shipped options are installable. Planned features may appear as roadmap
information, not install buttons. Start with a README download table; a service
is not required.

## Phase A: two build profiles

First implement Core (without images) and Images (Core plus images, the initial
recommended download). Proposed commands after FS-010:

```sh
cargo build -p foldshift-cli --release --no-default-features
cargo build -p foldshift-cli --release --no-default-features --features images
```

Make the CLI image dependency optional and gate help, dispatch, registration and
tests consistently. Use one feature-based source of truth for available
operations. CI must exercise each profile; workspace tests alone do not prove
minimality. Inspect the package-targeted dependency graph to show that Core has
no image codecs, distinguishing runtime and development dependencies.

Images are linked into the executable at this stage, not installed as a plugin.
Changing profiles replaces the executable with another tested build. Unchecking
a box cannot unload linked code or shrink an existing binary. Start with two
artifacts per supported platform, avoiding every possible codec combination.

## Phase B: optional engine packs

Introduce a minimal versioned subprocess protocol when PDF/media needs it.
Do not begin with native Rust dynamic plugins. Conversion workers stay offline;
explicit install/update networking lives in a separate component.

Proposed CLI (not implemented):

```text
foldshift packs list
foldshift packs install pdf
foldshift packs install media
foldshift packs install --from-file verified-pack.zip
foldshift packs remove pdf
```

People select capabilities; diagnostics expose the underlying engine/version.
Resolve shared dependencies before showing size. Audio and Video may need one
media download; selecting Audio alone does not automatically yield a smaller
engine. A separate audio-only build is justified only by measured benefit.

## Manifest contract

| Data | Purpose |
|---|---|
| Schema, pack ID/version | Stable identity and readable compatibility |
| OS, architecture, ABI | Reject wrong-target packages |
| Compatible core/protocol versions | Prevent incompatible execution |
| Capabilities and dependencies | Selection and shared-engine accounting |
| URL, download size, checksum | Exact artifact identity |
| Unpacked size and file inventory | Disk estimates and installation validation |
| Entrypoint, engine/build version | Execute the intended packaged tool |
| Licenses/notices | Explain terms for the actual build |
| Signature and trusted key ID | Authenticate the artifact manifest |

Use established signing software and a trust root shipped independently of the
manifest. A checksum from the same untrusted download does not authenticate it.

## Installation lifecycle

1. Resolve capabilities and compatible dependencies.
2. Show downloads, disk requirements, licenses and intended changes.
3. On an explicit action, download or read the offline archive into staging.
4. Verify trusted manifest, checksum, target/version and file inventory.
5. Enforce archive size bounds and reject escaping paths or unsafe links, using
   a mature archive library.
6. Install into a versioned per-user location and check engine health.
7. Atomically activate the version; failures retain the previous working copy.
8. Record selections, versions and provenance locally.

Use platform directory conventions through an established library; avoid admin
requirements. Jobs hold a specific engine version. Updates/removals cannot
invalidate active jobs; conflicting installs serialize. Remove only managed
files, keeping shared engines while another capability needs them.

Conversion and ordinary startup never initiate downloads or update checks.
Offline users import the same signed manifest/artifact without contacting GitHub.

## Configuration

Persist settings for shipped features: selected capabilities, resolved versions,
output defaults and later bounded concurrency. Flags override saved defaults.
Settings should be inspectable/resettable and versioned. Do not add this
configuration machinery to the first image-guard task.

## Release gates

- Minimal build works without engines; missing, disabled, incompatible and
  planned states are distinguishable.
- Size estimates come from artifacts and account for shared dependencies,
  unpacking and temporary update space.
- Test corruption, wrong targets, unsafe archive paths, disk exhaustion,
  cancellation, interrupted updates, active jobs and shared-engine removal.
- License review covers the exact packaged build. Subprocess separation does
  not remove license obligations.
- Publish Core/Images profiles before promising arbitrary custom installations.
  Runtime packs ship only after install/update/remove and offline tests pass.
