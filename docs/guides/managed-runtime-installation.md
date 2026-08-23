# Managed runtime installation

How GixGiz installs a local AI runtime for a user who has none, and why the
design looks the way it does.

Related: [ADR 0005](../adr/0005-windows-application-identity-and-packaging-direction.md),
[ADR 0006](../adr/0006-runtime-abstraction-and-ollama-provider.md),
[ADR 0007](../adr/0007-privileged-installer-helper-elevation-and-rollback.md),
[`ollama-runtime.md`](./ollama-runtime.md).

## Why there is no privileged helper

ADR 0007 defines a narrow privileged helper for operations that genuinely need
administrator rights. **The Ollama Windows installation does not**, so the helper
is deliberately not used and not built.

Evidence gathered from a real installation:

```text
Install location   %LOCALAPPDATA%\Programs\Ollama      (per user)
Registry           HKCU\...\Uninstall\"Ollama version <v>"
HKLM               no entry (both 64-bit and 32-bit roots enumerated)
Installer format   Inno Setup (unins000.exe / .dat / .msg)
Authenticode       CN=Ollama Inc., O=Ollama Inc., L=Toronto, S=Ontario, C=CA,
                   SERIALNUMBER=2713355, OID.2.5.4.15=Private Organization
                   Issuer: DigiCert G5 CS ECC SHA384 2021 CA1
```

ADR 0007 §1 is explicit: *"An operation that can succeed safely per-user must not
be elevated for convenience."* Building a privileged helper for an operation that
needs no privilege would add a permanent elevation surface for no user benefit,
so `requires_administrator` is `false` and no elevation is ever requested.

ADR 0007 remains accepted and unused. If a future runtime genuinely requires a
machine-wide change, the helper must be designed then, under that ADR.

## Approval is not the operating system's prompt

Two approvals are distinct, and only the first exists in these contracts:

| Approval | Meaning |
|---|---|
| **GixGiz plan approval** | The user reviewed one exact plan revision and accepted its described effects. |
| **OS elevation prompt** | Windows permitted a process to elevate. |

An elevation prompt can never substitute for a plan approval. For the current
per-user installation no elevation prompt occurs at all, and the plan approval is
still mandatory.

Approval binds to one **exact plan revision**. Regenerating the plan invalidates
any earlier approval, so a user can never approve one thing and have another run.

## Order of operations

```text
detect ──► existing runtime?  ──yes──► reuse it, offer no installation
   │
   no
   ▼
build reviewable plan (no system change)
   ▼
user approves the exact revision          ◄── nothing happens before this
   ▼
transfer artifact into GixGiz staging
   ▼
VERIFY: size bound + pinned SHA-256 + Authenticode publisher
   │
   ├─ rejected ──► quarantine, delete our staging, stop. Never executed.
   ▼
execute installer (re-verified immediately beforehand)
   ▼
INDEPENDENTLY VERIFY the installed runtime
   ▼
ownership = GixGizManaged     ◄── only now
```

An installer exit code is never sufficient evidence of anything. Ownership moves
to `GixGizManaged` only when **all five** independent checks pass:

```text
executable_located ∧ signature_valid ∧ publisher_matched
                   ∧ version_supported ∧ endpoint_healthy
```

Any missing check leaves ownership unchanged and the job in an attention state.

## Trusted source and integrity

The installer artifact is described by a **compiled manifest**. No caller can
supply an origin, artifact name, version, or digest, and no "latest" URL is
resolved at runtime.

```text
origin      https://github.com/ollama/ollama/releases/download/...
artifact    OllamaSetup.exe
digest      pinned SHA-256 from the release's sha256sum.txt
publisher   Ollama Inc. (Authenticode, normalized and compared exactly)
```

An empty or malformed digest makes the manifest untrusted, and installation is
refused rather than proceeding without integrity evidence. **The shipped manifest
currently has no digest pinned, so managed installation is refused until one is
added.** Populate `V0_1_INSTALLER.sha256` from the matching release's
`sha256sum.txt` to enable it.

GitHub release downloads redirect across hosts, so bounded redirects are allowed.
This is safe because integrity rests on the pinned digest and the Authenticode
publisher, not on the transport path: a hostile redirect cannot produce an
artifact that satisfies both.

## No new dependency, no `unsafe`

Transfer, hashing, and signature verification all run through the same fixed,
non-interactive system PowerShell already audited for Authenticode checks:

- resolved through a fixed `GLOBALROOT` path, verified to be a real file;
- launched with `-NoLogo -NoProfile -NonInteractive` and a pinned `PSModulePath`;
- every caller value passed as a hex token validated by `^x[0-9A-F]+$` and
  decoded inside the script, so an adversarial path or URL stays inert data;
- bounded output, bounded duration, and no shell interpolation anywhere.

This avoids adding a TLS stack or a Windows FFI dependency, and keeps
`#![forbid(unsafe_code)]` intact across the workspace.

## Staging and cleanup

Artifacts are staged in the GixGiz-owned staging root under the data root
(`%LOCALAPPDATA%\GixGiz\staging`). The staged path is always a direct child of
the canonicalized staging root; traversal, absolute paths, and nested names are
rejected.

Cleanup removes **only** the exact artifact GixGiz staged. It never touches the
provider installation, provider models, or any unrelated file.

## Ownership rules

| Situation | Ownership |
|---|---|
| Runtime installed by the user or another application | `External`, permanently |
| Runtime installed by this workflow and fully verified | `GixGizManaged` |
| Installation failed, uncertain, or unverified | unchanged |

An existing runtime is **reused, never replaced** — including when its version is
incompatible. Replacing an external installation would be an ownership transfer
and needs its own explicit design and approval.

## Recovery

Durable state lives in SQLite (`runtime_install_jobs`, migration 6). A unique
index permits at most one non-terminal job per provider, so an approved system
change cannot start twice.

On startup, before the transport accepts any request, an interrupted job is
reconciled through **fresh detection**, never by resuming. If the runtime now
verifies, the job becomes ready; otherwise it becomes an explicit attention state
with a `RecreatePlan` recovery route. An interrupted installation can neither
complete silently nor repeat a system change blindly.

## What is deliberately not implemented

- runtime update or automatic upgrade;
- runtime uninstall;
- machine-wide installation;
- replacing or reconfiguring an external installation;
- any privileged helper or elevated execution path.

## Testing

Every behaviour above is covered by deterministic tests that never download,
elevate, execute an installer, or change Windows installation state. The fake
installer records call order so tests assert the central property directly: a
rejected artifact never reaches execution.

```powershell
cargo test -p gixgiz-core runtime_install
cargo test -p gixgiz-runtime-ollama installer
cargo test -p gixgiz-persistence --test runtime_install_jobs
```

Real installation is not exercised by CI and must be validated manually on a
machine with no existing Ollama installation.
