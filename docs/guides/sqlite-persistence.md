# SQLite persistence development guide

## Ownership and boundary

`gixgiz-persistence` is the exclusive SQLite boundary. It owns connection
configuration, migrations, backups, health checks, and repository SQL.
`gixgiz-core` composes that service and maps its mandatory health into the
existing provider-neutral readiness contract. `gixgiz-desktop-host` performs
the blocking open on a Tokio blocking worker before serving the existing
authenticated handshake.

Flutter, loopback clients, providers, future Packs, and integrations must not
open the database, execute SQL, or depend on its schema. No transport route
returns raw SQL, database handles, or database paths.

## Data root

Production resolves the current user's local application-data directory:

```text
%LOCALAPPDATA%\GixGiz\
|-- data\gixgiz.db
|-- config\
|-- logs\
|-- cache\
|-- staging\
|-- content\
`-- backups\
```

The directories are created without elevation, canonicalized, and checked to
remain under the selected root. The database is never stored in the install
directory. Tests call `DataRoot::from_override` with a `tempfile::TempDir`; they
must never resolve the user's real `%LOCALAPPDATA%` tree.

Windows opens `data\gixgiz.owner.lock` with exclusive sharing for the lifetime
of the database owner. This prevents a second cooperating core process from
owning the same database while allowing crash recovery: the OS releases the
handle when the process exits. Files inherit the signed-in user's directory
permissions. Explicit installer-time ACL hardening remains a packaging task.

## SQLite selection and configuration

The foundation uses `rusqlite 0.40.1` with default features disabled and only
`backup` plus `bundled` enabled. Rust's standard library has no SQLite support.
Rusqlite and `libsqlite3-sys` are MIT licensed, the bundled SQLite engine is
public domain, and the stack is actively maintained. It supports Windows,
prepared statements, transactions, busy timeouts, WAL, and SQLite's online
backup API. Rusqlite is synchronous, so the sidecar opens it only on a blocking
worker. Bundling adds a native C compile and executable size, but it avoids
relying on an absent or incompatible machine SQLite DLL before Windows CI is
introduced. `tempfile` is an MIT/Apache-2.0 dev dependency used only for
isolated tests.

`sqlx` was not selected because its async runtime, pool, macros, and broader
database abstraction are unnecessary for one blocking local owner. `refinery`
was not selected because the small immutable migration ledger is implemented
directly with rusqlite transactions. The `sqlite` wrapper offered no narrower
advantage over the transaction, error-mapping, and online-backup APIs used from
rusqlite. Persistence does not add Tokio, an ORM, networking, or a provider
dependency.

Every open applies and verifies:

- `foreign_keys = ON`;
- `journal_mode = WAL`;
- a five-second production busy timeout;
- one in-process connection guarded inside the persistence crate.

Every repository write uses an immediate transaction and bound parameters.
Busy-timeout overrides are capped at 30 seconds, and audit reads require a
bounded page size. The raw `rusqlite::Connection` is never public.

## Schema and migrations

Repository migrations live under `crates/gixgiz-persistence/migrations/` and
are compiled into an ordered immutable list. Never edit a released migration;
add the next integer version.

Each migration transaction applies its SQL, appends `schema_migrations`, records
`last_migrated_application_version`, updates `PRAGMA user_version`, and commits
as one unit. A failure rolls back all schema and ledger changes. On open, the
runner validates the applied version/name ledger. A database newer than
`CURRENT_SCHEMA_VERSION` returns a safe incompatible-version failure. Automatic
downgrade is never attempted.

The current schema contains only:

- migration and application metadata;
- bounded non-secret settings;
- minimal provider-neutral durable-job metadata;
- append-only categorical audit events; and
- provider-neutral runtime ownership plus separate reuse and management consent.

Schema version 2 adds one `runtime_policy` table. It stores only a bounded
provider ID, ownership, reuse consent, management consent, and update time.
Runtime detection is read-only and never creates or changes this policy record.
Malformed or unknown persisted policy values fail closed. The schema contains
no executable path, endpoint, raw provider response, model inventory, download,
hardware, conversation, chat, installer, or Pack data.

## Backup and recovery

Migration descriptors classify irreversible or potentially lossy steps. Before
one runs against existing state, the SQLite online backup API creates a unique
snapshot under `backups\`. The snapshot must open, pass `quick_check`, and report
the expected prior schema version. Backup failure stops the migration. Recovery
does not overwrite the only known-good database or automatically downgrade it.

Normal startup reads the ledger and performs a no-op write inside an immediate
transaction that is explicitly rolled back. This verifies writability without
retaining probe data. Corrupt or non-SQLite files fail during open/configuration
and map to a stable integrity failure; backup verification uses the explicit
integrity check.

## Data and error policy

Repository APIs accept bounded UTF-8 text and typed identifiers/states. Setting
and metadata keys that indicate secrets or credentials are rejected. Audit
events accept only category, action, outcome, correlation ID, and timestamp;
there is no arbitrary message or payload field.

Do not store secrets, bearer tokens, prompts, conversation content, raw
diagnostics, logs, model binaries, downloads, installer packages, or large
blobs. Future content files belong under a validated content root and SQLite
may hold only bounded verified metadata references.

Filesystem and SQLite causes remain internal. Boundary-safe payloads and health
messages exclude paths, SQL, raw database text, and private record values.

## Development checks

Run from the repository root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p gixgiz-contracts --example generate_bindings -- --check
git diff --check
```

Migration tests cover a fresh version-zero database, the version 1 to 2 upgrade,
the complete currently supported upgrade path, ledger ordering, existing-data
preservation, failed transaction rollback, newer-schema refusal, and
backup-before-irreversible behavior. Add a fixture and upgrade test for every
future released schema version.
