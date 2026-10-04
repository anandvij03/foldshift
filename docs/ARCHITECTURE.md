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

Defines formats, media kinds, jobs, operation metadata, inspection results, and
stable error types. It has no filesystem engine and no network dependency.

### `foldshift-image`

Implements the initial pure-Rust image capability. It owns image format mapping,
inspection, proportional resize, encoding, and safe output finalization.

### `foldshift-cli`

Parses human input and calls capability functions. It contains no conversion
logic.

## Future engine boundary

Large or native capabilities should implement an adapter behind the core job
model. PDF and media engines may be subprocesses. This makes crashes and
licensing boundaries easier to isolate and keeps the core distribution honest.

```text
Job -> validation -> restricted worker -> temporary output -> validation -> move
```

## Compatibility contract

This is a design direction. The CLI currently calls capability functions
directly; the serializable `Job` type is not an implemented execution protocol.

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
