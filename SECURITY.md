# Security policy

File converters parse untrusted input. Treat every input as potentially hostile.

## v0.1 guarantees

- The conversion crates contain no runtime networking code.
- Existing output files are not overwritten unless the caller passes `--force`.
- Output is encoded to a sibling temporary file first.
- The temporary file is removed after an encoding or finalization failure when
  possible.

## Known limitations

Version 0.1 runs decoding in the main process. It does not yet enforce worker
process isolation, CPU or memory ceilings, filesystem sandboxing, or timeouts.
Do not market the current release as sandboxed.

Before PDF or media engines are enabled by default, they should run in restricted
workers with no network access and access limited to the input and a temporary
directory.

## Reporting a vulnerability

Use the repository's **Security / Advisories / Report a vulnerability** option
when private vulnerability reporting is enabled. Include the version, operating
system, impact, and a minimal synthetic reproduction.

If that option is unavailable, open an issue asking for a private security
contact, without vulnerability details or exploit files. Do not post sensitive
documents or working exploits in public issues.

Only the current development branch is maintained at present. Response and fix
times depend on maintainer availability; there is no support SLA.
