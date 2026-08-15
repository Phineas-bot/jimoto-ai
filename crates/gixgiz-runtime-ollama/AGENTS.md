# AGENTS.md - Ollama runtime adapter policy

> Applies to `crates/gixgiz-runtime-ollama/` in addition to repository and `crates/` policy.

- Keep every Ollama executable name, command argument, environment variable, route, payload,
  version rule, and model tag inside this crate.
- Connect only by direct TCP to a validated loopback address. Do not use proxies, redirects,
  DNS-derived remote addresses, TLS, or arbitrary caller-supplied URLs.
- Read-only detection, version, health, and model listing must not adopt or mutate an external
  installation.
- Never start, stop, or restart an external process. Lifecycle control is limited to the exact
  child handle created by this adapter after provider-neutral ownership and consent checks.
- Pass executable paths and fixed arguments directly to `Command`; never invoke a shell.
- Bound process duration/output and HTTP duration/body size. Keep raw provider bodies and local
  model inventories out of logs and boundary errors.
- Keep deterministic tests independent from a live installation. Real-provider smoke tests must
  be ignored, explicitly environment-gated, and loopback-only. Any mutating setup smoke requires
  a separate explicit acquisition gate and must retain rather than delete provider-owned data.
- Do not add `unsafe` Rust, Windows registry/service control, installation, update, uninstall,
  model deletion, arbitrary model pull, or general inference/chat behavior. Task 10 may pull only
  an allowlisted approved artifact and run only its fixed bounded readiness inference.
