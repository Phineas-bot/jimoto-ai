# Runtime abstraction agent rules

These rules apply to `gixgiz-runtime` in addition to the repository and Rust
workspace policies.

- Keep every public type and trait provider-neutral. Provider endpoints,
  executable names, commands, payloads, tags, and error text belong in adapter
  crates.
- Runtime operations must accept bounded deadlines and cooperative
  cancellation. Cancellation is distinct from failure.
- Ownership and reuse/management consent are independent of detection.
- Do not add transport, persistence SQL, Flutter, installation, downloads, or
  inference behavior here.
- Fake-provider utilities must remain deterministic and perform no system or
  network access.
