# Architecture Decision Records

Architecture Decision Records (ADRs) preserve the context and consequences of important GixGiz engineering decisions.

## Naming

Use sequential, descriptive names:

```text
0001-modular-monolith-and-process-topology.md
0002-windows-flutter-rust-baseline.md
0003-flutter-rust-local-transport.md
```

## Required structure

```markdown
# ADR NNNN: Decision title

- Status: Proposed | Accepted | Superseded | Deprecated
- Date: YYYY-MM-DD
- Owners: names or team

## Context

## Decision

## Alternatives considered

## Consequences

## Security and privacy impact

## Implementation constraints

## Follow-up work
```

## Accepted decisions

These ADRs govern the current foundation and runtime work. Read the applicable decisions before changing the desktop shell, Rust core, local boundary, persistence, Windows identity, runtime providers, or privileged setup behavior.

| ADR | Decision | Primary task impact |
|---|---|---|
| [0001](./0001-modular-monolith-and-process-topology.md) | Modular monolith with Flutter desktop and supervised Rust core sidecar | Tasks 02–05 |
| [0002](./0002-windows-flutter-rust-baseline.md) | Windows 11 x64, Flutter stable desktop and stable Rust core baseline | Tasks 02–03 |
| [0003](./0003-flutter-rust-local-transport.md) | Authenticated loopback typed API with version handshake and streaming | Task 04 |
| [0004](./0004-sqlite-persistence-and-migrations.md) | Core-owned SQLite with transactional migrations and recovery | Task 05 |
| [0005](./0005-windows-application-identity-and-packaging-direction.md) | Stable GixGiz Windows identity, per-user installation and packaging direction | Tasks 02–05 |
| [0006](./0006-runtime-abstraction-and-ollama-provider.md) | Provider-neutral runtime abstraction with explicit Ollama adapter ownership and consent | Tasks 09–10 |
| [0007](./0007-privileged-installer-helper-elevation-and-rollback.md) | Least-privilege helper boundary with structured approval, elevation and rollback | Tasks 09–10 |

## Deferred ADR backlog

The following decisions remain intentionally deferred until their implementation stage:

- Final Windows installer format and automatic update mechanism.
- Public local gateway and external-client registration.
- Tool runtime and sandbox architecture.
- AI Pack format, signing and isolation.

Create or update an ADR before implementing a decision that is difficult to reverse, cross-cutting, security-sensitive, externally visible or likely to be questioned later. A later ADR may supersede an accepted decision, but implementation must not silently diverge from it.
