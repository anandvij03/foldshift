# Foldshift: GitHub launch checklist

Repository created and cloned: https://github.com/anandvij03/foldshift.
The existing initial commit and README have been preserved. Project source has
been imported locally; no import commit or push has been performed yet.
`origin` already exists: skip the `git remote add origin` example below.
Review the current GitHub settings against the checklist; they have not been
inspected remotely.

## 1. Create the repository — owner

- [ ] Create `foldshift` under your personal GitHub account. An organization can
  be added later if the project needs one.
- [ ] Choose **Public**.
- [ ] Description: **Lightweight, offline file conversion. Built in Rust.**
- [ ] Leave GitHub's **Add README**, **Add .gitignore**, and **Choose a license**
  options off. These files already exist locally; start with an empty remote.
- [ ] Send the repository URL to the collaborator preparing the first push.

The source uses MIT. GitHub will detect the checked-in `LICENSE` after upload.
Suggested topics: `rust`, `file-converter`, `offline`, `privacy`, `cli`,
`image-processing`. Advertise currently implemented features; describe PDF and
media as planned.

## 2. Prepare the first push — local work

- [x] Adopt Foldshift and `foldshift` as the CLI command.
- [x] Initialize the local Git repository on `main`.
- [x] Include source, dependency lockfile, README, license, and roadmap.
- [x] Add contribution guidance, issue forms, PR template, and changelog.
- [x] Add cross-platform checks and weekly Dependabot configuration.
- [x] Exclude toolchains, build outputs, generated downloads, and `.env` files.
- [x] Confirm the repository owner and URL; add `repository` metadata to crates
  and repository-specific links to the README.
- [ ] Confirm Git commit name/email (a GitHub noreply address can be used).
- [ ] Review the exact staged file list and diff before committing.
- [ ] Commit and push `main` to the empty repository.

Commands below are for the project root; replace `OWNER` with the actual owner.
Initialize with `git init -b main` only if this folder is not already a repository.

```sh
git add .
git diff --cached --stat
git diff --cached
git commit -m "Initial Foldshift image converter"
git remote add origin https://github.com/OWNER/foldshift.git
git remote -v
git push -u origin main
```

If `origin` already exists, inspect it before changing it. Do not overwrite a
remote or force-push over an existing history.

## 3. Configure GitHub — owner, after first push

- [ ] Confirm `main` is the default branch, Issues and Actions are enabled.
- [ ] Confirm the README renders and GitHub detects the MIT license.
- [ ] Wait for the first Windows, macOS, and Linux checks to pass.
- [ ] Enable private vulnerability reporting under repository security settings.
  Confirm that a **Report a vulnerability** entry is available.
- [ ] Enable dependency graph, Dependabot alerts, and security updates where
  available. The committed config schedules normal version-update PRs.
- [ ] Keep Actions workflow-token permissions read-only by default. CI does not
  need repository secrets or write access.
- [ ] Protect `main`: require CI checks, block force pushes and branch deletion,
  and require conversation resolution. Select the actual check names from the
  first run: `Rust checks (ubuntu-latest)`, `Rust checks (windows-latest)`, and
  `Rust checks (macos-latest)`.
- [ ] Require PRs for changes. While you are the only maintainer, do not require
  another person's approval for your own PRs; add a review requirement when a
  second maintainer joins.
- [ ] Enable squash merge and automatic deletion of merged topic branches.
- [ ] Add a short About description and the suggested topics.

Optional later: Discussions, a project board, website, CODEOWNERS, a formal code
of conduct with a designated reporting contact, and package-manager listings.

## 4. Initial issues and release

- [ ] Create a `v0.1 preview` milestone.
- [ ] File focused issues for animation/multi-page safeguards, orientation and
  color handling, batch conversion, and reproducible release packaging.
- [ ] File subsequent milestones for PDF tools, media engines, and the desktop UI.
- [ ] Follow [RELEASING.md](RELEASING.md) before tagging or attaching binaries.

Current local archives under `outputs/` are not automatically public releases.
The older LocalConvert archive predates the Foldshift rename and should not be
uploaded as the Foldshift release.

## Reference

- [GitHub: import existing local code](https://docs.github.com/en/migrations/importing-source-code/using-the-command-line-to-import-source-code/adding-locally-hosted-code-to-github)
- [GitHub: protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [GitHub: private vulnerability reporting](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting/configure-for-a-repository)
