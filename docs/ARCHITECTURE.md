# Architecture

## Principles

1. The Rust core stays small and contains no codec implementations.
2. Capabilities are explicit. Missing engines produce clear errors.
3. CLI, desktop UI, scripts, and any future local API submit the same job model.
4. Conversion has no network path. Updater and capability installers remain
   separate components.
5. A completed output replaces nothing until encoding succeeds.

## Crates

### `foldshift-core`

Defines media kinds, inspection results, operation metadata, and stable error
types. It has no filesystem engine and no network dependency. Speculative general
job/format abstractions are deferred until Milestone 3/4 introduces engine packs
and general batch execution.

### `foldshift-image`

Implements the initial pure-Rust image capability. It owns image format mapping,
inspection, proportional resize, encoding, and safe output finalization.

### `foldshift-cli`

Parses human input and calls capability functions. It contains no conversion
logic.

## Same-file and alias policy

1. **No-Force Protection**: When the requested output matches the input file (or
   any existing destination or canonical path alias), conversion halts early with
   `OutputExists` before reading or decoding. Original files remain untouched.
2. **Atomic In-Place Replacement**: When `--force` is provided targeting the
   input file or an alias, the input is read completely into memory first. The
   new output is encoded to an isolated temporary file (`NamedTempFile`) in the
   target directory.
3. **Failure Preservation**: If any encoding, option validation, or format rule
   fails (such as attempting JPEG conversion with transparent alpha or invalid
   quality), the temporary file is discarded and the original input file is
   preserved byte-for-byte without truncation.
4. **Publication**: The temporary file is atomically persisted over the destination
   only after encoding and disk sync succeed.

## Future engine boundary

Large or native capabilities should implement an adapter behind the core job
model. PDF and media engines may be subprocesses. This makes crashes and
licensing boundaries easier to isolate and keeps the core distribution honest.

```text
Job -> validation -> restricted worker -> temporary output -> validation -> move
```

## Compatibility contract

This is a design direction. The CLI and Web UI currently call capability functions
directly; a generalized `Job` type is deferred until milestone 3/4.

Operation IDs are namespaced (`image.convert`, `image.resize`, `pdf.merge`).
Options are serialized values so the same request can travel from a UI to the
core. Before a public 1.0 release, jobs need a schema version and compatibility
policy.

## Selectable installation

Start with Core and Images build profiles. Images are currently mandatory and
must become optional before advertising a core-only download. Later PDF/media
engines use versioned, explicitly installed subprocess packs.

See [CAPABILITIES-AND-DOWNLOADS.md](CAPABILITIES-AND-DOWNLOADS.md) for profile
replacement, dependency resolution, sizes, offline import and update recovery.
Selections reflect actual tested artifacts. The UI is a separate optional download.
