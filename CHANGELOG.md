# Changelog

## Unreleased

### Added

- Foldshift CLI for still-image conversion, proportional resizing, inspection,
  and JSON capability reporting.
- PNG, JPEG, WebP, GIF, BMP, and TIFF support through the `image` library.
- Output preservation and temporary-file publication through `tempfile`.
- Tests for conversion, resizing, corrupt input, and overwrite behavior.
- CI configuration for Windows, macOS, and Linux; contributor and issue templates.

### Changed

- Adopted Foldshift as the project and command name, replacing the LocalConvert
  prototype and its `fc` command.

### Known limitations

- Still images only: animations and multi-page images are flattened.
- Embedded orientation and color profiles are not applied or preserved.
- Quality is JPEG-only; WebP output is lossless; transparent JPEG export fails.
- PDF, media, desktop UI, and isolated workers are not implemented.
