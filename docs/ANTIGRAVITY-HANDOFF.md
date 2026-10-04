# Antigravity assignment: FS-001

This file is a bounded brief for the maintainer to pass to Antigravity. Creating
it does not start an external agent.

## Read first

Read `CONTRIBUTING.md`, `docs/ROADMAP.md`, `docs/DEVELOPMENT-WORKFLOW.md`,
`docs/CAPABILITIES-AND-DOWNLOADS.md` and `docs/REVIEW-CHECKLIST.md`.
Inspect actual status/instructions before editing. Planning used commit
`64f0b56`; do not reset newer work to it.

## Objective and scope

Make still-image conversion refuse silent content loss and fail invalid
requests before expensive decoding. Keep this to one reviewable change.

1. Detect animated/multi-frame inputs and multi-page TIFF before conversion or
   resize discards content. Cover enabled formats, including GIF, TIFF, WebP
   and animated PNG where applicable. Prefer existing decoder APIs or a narrowly
   justified library; do not write general format parsers. If reliable detection
   is unavailable, document a conservative explicit error policy.
2. Retain single-frame GIF and single-page TIFF support where distinguishable.
   Return an actionable error for unsupported multi-frame/page content. Animation
   preservation and a frame-export interface are outside this task.
3. Preflight destination format, quality range, output collision and resize
   dimensions before full decode. Keep final no-clobber publication checks too;
   early validation alone does not prevent a race.
4. Invalid requests must not create output directories or files.
5. Document behavior and add targeted regression tests.

## Likely files

- `crates/foldshift-image/src/lib.rs` and focused helpers/tests if necessary.
- `crates/foldshift-cli/tests/cli.rs` for user-visible errors.
- `PROJECT-OVERVIEW.md` and `CHANGELOG.md` for changed guarantees.

Explain changes to public core types, dependencies or command syntax in the
handoff. The GUI, installers, profile system, PDF and media engines are out of scope.

## Acceptance

- Synthetic animated/multi-page fixtures fail clearly with no output. Existing
  destinations remain byte-for-byte intact on rejected input, including `--force`.
- Ordinary still-image conversion and resize continue to work.
- Library quality validation exists independently of CLI parsing.
- Invalid destination formats/dimensions have no output filesystem side effects.
- Error precedence is deliberate and tested where it changes.
- Detection uses content, not only the extension.
- Any new dependency has a stated purpose, license and size assessment.
- Run format, Clippy and workspace tests with the lockfile; state platforms not
  tested rather than claiming universal success.

## Return for review

Use the handoff template in `DEVELOPMENT-WORKFLOW.md`: branch/commit, base,
changed files, test results, fixture list, limitations and measured/unmeasured
footprint. Stop writing the checkout before Codex cleanup.

Codex reviews preservation of data, validation, copying/allocations, dependencies
and simplicity. The maintainer decides when to merge.
