# Release checklist

Use a preview release while still-image limitations remain. Repository creation,
tagging, and public release publication are separate steps.

- [ ] Pick a version; keep the workspace version, changelog, tag, and executable
  `--version` consistent. For a first preview, consider `0.1.0-alpha.1` and tag
  `v0.1.0-alpha.1`.
- [ ] Update `Cargo.lock` through Cargo after changing the version.
- [ ] Run formatting, Clippy, tests, and release builds with `--locked`.
- [ ] Require passing GitHub CI for the exact commit being released.
- [ ] Build each advertised OS/architecture natively or with a verified toolchain.
  Do not publish untested platforms solely because a CI workflow exists.
- [ ] Check conversion, resizing, inspection, and overwrite failures using the
  packaged executable on each supported platform.
- [ ] Reopen converted files with an independent reader.
- [ ] Check runtime dependencies on a machine without the development toolchain.
- [ ] Bundle license notices for included dependencies and engines. Record
  dependency versions and build toolchain in build notes.
- [ ] Include a short quickstart, exact platform/architecture, and known limits.
- [ ] Name archives `foldshift-VERSION-OS-ARCH.zip` (or `.tar.gz`).
- [ ] Compute SHA-256 hashes after packaging and attach a checksum file. Checksums
  alone do not authenticate a publisher; describe signing status accurately.
- [ ] Create the version tag for that commit and draft a GitHub release.
- [ ] Mark it as a pre-release, review the uploaded files and notes, then publish.
- [ ] Test download, unpacking, and execution from the published release page.

Current limitations to carry in release notes: single-frame/page processing,
metadata/orientation/color-profile limitations, JPEG-only quality control, no
isolated workers, and no PDF, audio/video, or GUI yet. Do not promise a universal
converter or sandboxing in this release.

Normal users should download a release rather than the development toolchain.
Never include `work/`, Cargo caches, personal files, or developer credentials in
release archives. Keep binaries out of source commits.
