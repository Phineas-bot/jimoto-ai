# Contributing to GixGiz

Thank you for helping improve GixGiz. GixGiz is a Windows-first, local-first
AI operating platform. The current v0.1 goal is a guided desktop experience
that takes a supported non-technical Windows user from installation to a
verified response from a local model without requiring a terminal.

This guide explains how to make a contribution that can be reviewed and
merged safely. Read the repository's [README](./README.md),
[AGENTS.md](./AGENTS.md), relevant [task specifications](./docs/tasks/),
accepted [ADRs](./docs/adr/), and tests before changing code. `AGENTS.md` is
the authoritative engineering policy; deeper `AGENTS.md` files add rules for
their directories.

## Contribution scope

Keep each change focused on one coherent capability or documentation outcome.
Avoid broad "build the platform" pull requests, speculative frameworks, and
unrelated formatting or refactoring. An issue should make the included scope,
explicit non-goals, acceptance criteria, validation, and documentation or ADR
impact clear before implementation begins.

Discuss a proposal before starting work when it changes architecture, a public
or cross-process contract, database compatibility, security or privacy
behavior, runtime/provider behavior, installation/elevation, or Windows
packaging. Record difficult-to-reverse, cross-cutting, security-sensitive, or
externally visible decisions in an ADR rather than embedding the decision only
in code or a pull request.

## Conduct

The repository does not currently have a separate `CODE_OF_CONDUCT.md`. Until
one is adopted, everyone participating in issues, pull requests, reviews, and
other project spaces is expected to:

- Be respectful, professional, and constructive.
- Welcome good-faith contributions and differing viewpoints.
- Focus feedback on the work, not the person.
- Avoid harassment, discrimination, personal attacks, threats, or sharing
  another person's private information.

If conduct creates a safety or moderation concern, contact a repository
maintainer privately using a published GitHub contact method. Do not turn the
public issue tracker into a venue for personal disputes. Maintainers may ask
for a change in behavior, remove content, or limit participation to protect a
safe and productive project space.

## Security, privacy, and secrets

Do not report a suspected vulnerability through a public issue or pull request.
Use the repository's private vulnerability-reporting option if it is enabled;
otherwise contact a maintainer privately through their published GitHub contact
method. Share only the information needed to reproduce and assess the issue,
and do not publish a proof of concept before maintainers have had an
opportunity to respond.

Never commit or attach:

- API keys, credentials, tokens, passwords, private keys, certificates, or
  signing material;
- transport/bootstrap secrets, `.env` files, local databases, logs, prompts,
  conversations, personal files, or unsanitized diagnostics;
- downloaded models, model binaries, build output, or unsigned release
  artifacts.

Treat model output, repository content, files, and web content as untrusted
input. Preserve GixGiz's local-first boundaries: do not expose local services
beyond loopback, weaken authentication, add broad filesystem or shell access,
or log private content. If a secret is exposed, revoke or rotate it promptly
and notify a maintainer privately.

## Local setup

GixGiz development is supported on Windows 11 x64. Install the following
prerequisites before attempting a full local validation:

- Git.
- Flutter stable 3.44.8 with Windows desktop support enabled.
- Visual Studio 2022 or Visual Studio Build Tools with the **Desktop
  development with C++** workload and a Windows SDK.
- Rustup with the repository-pinned Rust 1.97.1 MSVC toolchain. Cargo reads
  [`rust-toolchain.toml`](./rust-toolchain.toml) automatically when Rustup is
  available.

For a repository you can write to, clone it and create a focused branch from
the current `main` branch:

```powershell
git clone https://github.com/Phineas-bot/gixgiz.git
Set-Location gixgiz
git switch main
git pull --ff-only origin main
git switch -c docs/short-description
```

If you do not have write access, fork the repository, clone your fork, and
open a pull request from that fork to `Phineas-bot/gixgiz:main`. Keep your
working tree clean before starting new work:

```powershell
git status --short
```

## Repository ownership and boundaries

The current Rust dependency direction is:

```text
gixgiz-desktop-host -> gixgiz-runtime-ollama -> gixgiz-runtime -> gixgiz-contracts
                    -> gixgiz-core -----------^         |
                                      -> gixgiz-persistence -> gixgiz-contracts
```

- Flutter owns presentation, accessibility, localization, navigation, and
  user intentions. It does not own provider, installer, hardware, business, or
  persistence policy.
- Rust contracts own versioned cross-process schemas. Regenerate checked Dart
  bindings from the Rust source of truth; do not hand-edit generated bindings.
- `gixgiz-persistence` exclusively owns SQLite connections, migrations,
  backups, and SQL.
- `gixgiz-runtime` owns provider-neutral runtime behavior; concrete provider
  endpoints, commands, payloads, and model tags belong in the corresponding
  adapter crate.
- `gixgiz-core` owns runtime ownership and consent policy. Detection and reuse
  approval must not silently grant management authority.
- The desktop host owns only the supervised, authenticated loopback sidecar
  transport and concrete adapter composition. It must not acquire platform
  policy or proxy raw provider APIs.

Follow the nearest applicable `AGENTS.md` for detailed ownership, security,
and validation rules. Do not add a dependency, break a public contract, change
migration compatibility, or introduce privileged behavior without the review
and approval required there.

## Issues and planning

Search existing issues and pull requests before opening a new issue. Use the
repository's **Implementation task** issue form for planned engineering work.
Complete its context, desired behavior, scope, non-goals, architecture/security
constraints, acceptance criteria, validation, and documentation/ADR impact.

For a defect report, include a minimal reproducible sequence, expected and
actual behavior, affected GixGiz version or commit, Windows version, and
sanitized diagnostics. Do not include secrets, private paths, prompt content,
or personal data. For an enhancement, explain the user value, supported
workflow, scope, non-goals, and any relevant specification or ADR.

## Branches and commits

- Branch from an up-to-date `main` branch; never commit directly to `main`.
- Use one branch and one focused change set per task. A descriptive name such
  as `docs/add-contributing-guide`, `fix/startup-timeout`, or
  `feat/hardware-scan` is preferred.
- Keep commits reviewable, buildable where practical, and limited to one
  logical concern. Do not mix drive-by formatting with functional work.
- Use an imperative Conventional Commit-style subject:

  ```text
  type(scope): concise summary
  ```

  For example: `docs(contributing): add contributor guide`. Choose a specific
  scope, keep the header at or below 72 characters, and do not end it with a
  period. Common types include `docs`, `feat`, `fix`, `test`, `refactor`,
  `build`, `ci`, and `chore`.

## Develop, test, and validate

Make the narrowest relevant change, then run the applicable checks. The full
local equivalent of the required Windows CI job is:

```powershell
# From the repository root
cargo run -p gixgiz-contracts --example generate_bindings -- --check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# From apps/desktop
flutter pub get
flutter gen-l10n
flutter analyze
flutter test
flutter build windows
```

Run `git diff --check` for every contribution. If a code change affects Rust
contracts, persistence, Flutter, packaging, or transport, run the relevant
targeted tests as well as the full checks above. Do not weaken assertions,
hide flaky tests with retries, use live Ollama or real hardware in deterministic
CI tests, or claim a check passed unless it actually ran.

In the pull request, provide testing evidence: exact commands, results,
targeted scenarios covered, Windows/toolchain context when relevant, and any
checks not run with the reason and remaining risk. Include screenshots or a
short recording for user-visible UI changes, and describe manual verification
for behavior that cannot be covered by automated tests.

## Documentation, generated artifacts, and migrations

Update documentation in the same pull request when behavior, interfaces,
setup, permissions, user-visible states, validation, or operations change.
Choose the authoritative document rather than duplicating information:

- Update the README for contributor-facing setup or workflow changes.
- Update task specifications when their defined scope or evidence changes.
- Add or update an ADR for an architectural decision and its consequences.
- Update guides when a supported operational procedure changes.

When changing a Rust-owned contract, regenerate the checked schema and Dart
bindings through the repository's generator and include the resulting intended
files. For SQLite, add a new ordered, immutable migration instead of editing
an applied migration, and add the required migration and recovery tests.

## Pull requests

Open a pull request against `main` only after reviewing the complete diff.
Use a draft PR while work is incomplete. Keep the pull request focused and
link the related issue without using issue-closing language unless the issue
is actually ready to close.

Every pull request should state:

- What changed and why, including explicit non-goals.
- The related issue or task specification.
- User-visible, contract, migration, security/privacy, compatibility, and
  documentation impact; say "none" where appropriate.
- Exact validation and testing evidence, plus anything not run and why.
- Known risks, limitations, and a safe rollback approach when the change has
  operational impact.

Do not include unrelated changes, generated build output, or sensitive data.
Address CI failures and review feedback with additional focused commits. Do
not force-push over another contributor's work or rewrite shared history.

## Maintainer review and merge

Maintainers review the complete diff for task scope, architecture and ADR
alignment, ownership boundaries, security and privacy, test evidence,
documentation, and CI results. They may request clarification, narrower scope,
additional tests, documentation, an ADR, or a split into smaller pull requests.

CI and review are evidence for a maintainer's merge decision; they are not a
guarantee that a change is ready. Maintainers decide when a pull request is
ready, merge it, and manage releases. Do not merge, tag, sign, publish, or
change release channels unless you have explicit repository authority.

## License

GixGiz is available under the [MIT License](./LICENSE). Keep added code,
assets, and documentation compatible with that license and preserve any
required third-party notices or attribution.
