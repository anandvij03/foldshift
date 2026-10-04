# Contributing to Foldshift

Foldshift processes files on the user's device. Contributions should help make
that workflow small, predictable, and easy to use.

## Development setup

Install current stable Rust (minimum declared version: 1.88), clone the
repository, and run the following from its root:

```sh
cargo build --locked
cargo run -p foldshift-cli -- capabilities
cargo test --workspace --locked
```

Windows builds using the standard MSVC toolchain also require the Visual Studio
C++ build tools and Windows SDK. These are development prerequisites; users of
a packaged binary do not need Rust or those build tools.

Before submitting a pull request:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Commit `Cargo.lock` whenever dependencies change. CI runs on Windows, Linux,
and macOS. Workspace-local tools in `work/` are development conveniences and
are not required by, or committed to, the project.

## What to work on

See [the roadmap](docs/ROADMAP.md). Open an issue before adding a capability,
large dependency, or changing command syntax. Small bug fixes and documentation
improvements can go directly to a pull request.

Prefer established libraries for codecs, argument parsing, temporary files,
and platform integration. Explain why a new dependency is needed, its license,
and its effect on binary size and supported platforms. Never place networking
or telemetry in the conversion path.

Keep conversion behavior in capability crates. The CLI and eventual GUI should
share it. Preserve originals on errors and require an explicit overwrite option.
Tests should cover user-visible behavior such as reopening outputs, transparency,
dimensions, and failure recovery.

## Issues and pull requests

Provide the version, operating system, command, expected result, and actual
result. Attach only synthetic or redacted files you have permission to share.
Do not post documents containing private data. For vulnerabilities, follow
[SECURITY.md](SECURITY.md).

Keep pull requests focused. Explain the problem, resulting behavior, and tests.
Screenshots are useful for UI changes; sample output is useful for CLI changes.
Discuss code and ideas respectfully. Harassment and personal attacks are not
welcome; maintainers may moderate discussions or close abusive contributions.

## License

Project contributions use the repository's MIT license. Preserve third-party
license notices and identify the source of copied or adapted code. No separate
contributor agreement is required at this stage.
