# Capability recommendation development

Task 08 converts a versioned `MachineProfile` and user intent into a deterministic, provider-neutral `CapabilityReport`. The engine runs locally in `gixgiz-core`; Flutter sends typed preferences and renders the result. Recommendation code does not query Windows, SQLite, a runtime provider, an LLM, or a remote catalogue.

## Ownership and flow

```text
MachineProfile + UserPreferenceProfile
        -> hard-constraint filter
        -> deterministic preference score
        -> stable catalogue-ID tie-break
        -> CapabilityReport
        -> Flutter presentation
```

- `gixgiz-contracts` owns versioned request, response, plan, reason, warning, resource, confidence, and catalogue identity types.
- `gixgiz-core::capability` owns the static catalogue, safety rules, scoring, estimates, and explainability policy.
- `gixgiz-desktop-host` exposes `POST /internal/v1/recommendations` through the existing authenticated, post-handshake loopback boundary.
- Flutter owns workload and preference controls, transient request state, accessibility, and user-facing presentation. It does not reimplement rules.

The operation is synchronous and bounded because it evaluates six in-memory candidates. It consumes the supplied profile and never starts or reuses a hardware scan.

## Versioned catalogue

The compiled v0.1 catalogue version is `gixgiz-catalogue-v0.1.0`. Stable model IDs are provider-neutral and contain no Ollama tags or installation identifiers. The runtime entry `gixgiz.local-text-runtime.v1` is planning metadata, not an installed provider or a promise that an adapter is available.

| Workload | Candidate | Size class | Licence | Source |
|---|---|---|---|---|
| Writing and chat | Qwen 2.5 Compact | 0.5B | Apache-2.0 | `Qwen/Qwen2.5-0.5B-Instruct` |
| Writing and chat | Qwen 2.5 Everyday | 1.5B | Apache-2.0 | `Qwen/Qwen2.5-1.5B-Instruct` |
| Writing and chat | Qwen 2.5 Quality | 7B | Apache-2.0 | `Qwen/Qwen2.5-7B-Instruct` |
| Coding | Qwen 2.5 Coder Compact | 0.5B | Apache-2.0 | `Qwen/Qwen2.5-Coder-0.5B-Instruct` |
| Coding | Qwen 2.5 Coder Everyday | 1.5B | Apache-2.0 | `Qwen/Qwen2.5-Coder-1.5B-Instruct` |
| Coding | Qwen 2.5 Coder Quality | 7B | Apache-2.0 | `Qwen/Qwen2.5-Coder-7B-Instruct` |

Each contract entry carries its display name, family, workload, size class, SPDX licence, and HTTPS provenance page. It deliberately contains no download URL. Catalogue metadata is compiled into the signed application build; Task 08 adds no remote update path.

## Rule and estimate policy

The rule-set version is `gixgiz-capability-rules-v0.1.0`. Every report and every returned plan records the catalogue and rule-set versions. Reports also record the input profile schema, preferences, and the source scan timestamp. Reusing the scan timestamp avoids a wall-clock input, so identical profiles, versions, and preferences serialize identically.

Hard filtering happens before scoring. A candidate is removed when reliable evidence violates architecture, logical-processor, total-memory, available-memory, or selected-storage requirements. Required memory and storage must both retain their safety margin. Unknown architecture, processor count, total memory, or free storage blocks selection. Unknown video memory or acceleration is never counted; CPU-capable candidates may remain available with lower confidence and explicit warnings.

| Size class | Working memory | Memory margin | Installed storage | Storage margin | Planning context |
|---|---:|---:|---:|---:|---:|
| Compact | 2 GiB | 2 GiB | 1 GiB | 2 GiB | 4,096 tokens |
| Standard | 4 GiB | 3 GiB | 2 GiB | 2 GiB | 8,192 tokens |
| Large | 8 GiB | 4 GiB | 6 GiB | 3 GiB | 8,192 tokens |

These are conservative planning allowances, not benchmark results, throughput promises, exact package sizes, or provider installation requirements. Optional GPU use is selected only when both acceleration and sufficient dedicated memory are reliably evidenced. All v0.1 candidates retain a processor-only plan.

After hard filtering, preference scoring ranks only safe candidates:

- `balanced` favors the standard size;
- `fastest_setup` favors smaller safe candidates;
- `lowest_resource_use` preserves the most headroom;
- `best_quality_within_safe_limits` favors the strongest safe candidate;
- workload matching is scored before size preference;
- equal scores use stable catalogue identity as the final tie-break.

## Confidence and explanations

`High` means critical evidence is reliable and optional uncertainty is limited. `Medium` means a plan is still safe but optional video-memory or acceleration evidence is unknown. `Low` means material optional evidence or currently available memory is unknown, or a no-plan result depends on missing critical evidence. Unknown values remain explicit in warning and reason codes.

Reasons explain workload match, safety margins, operating mode, preference selection, fallback role, and hard rejection. Warnings cover partial evidence, unknown available memory, video memory or acceleration, conservative processor-only operation, removable storage, limited fallback choices, unavailable larger choices, and explicit no-plan results. Messages are bounded, provider-neutral, and contain no raw Windows output or paths.

User-facing terminology intentionally uses "writing and chat," "coding," "memory allowance," "storage allowance," "standard," "accelerated," "smaller fallback," and "larger option." Provider identifiers and quantization details are not required to choose a plan.

## Persistence policy

Task 08 returns reports through transport and keeps them in transient Flutter state. It does not add a migration or repository because no setup workflow has approved a selected plan yet, and the Task 07 privacy policy does not persist raw `MachineProfile` evidence. Task 10 can persist the approved, bounded plan metadata through `gixgiz-persistence`, including its catalogue/rule versions and safe reasons/warnings, without storing the raw machine profile.

## Security and limitations

- Planning is local and uploads no machine evidence.
- The endpoint requires the per-launch bearer token and a compatible handshake, preserves correlation/request IDs, and maps errors to safe payloads.
- No secrets, prompts, conversations, hardware identifiers, raw paths, model binaries, or provider output are stored or logged.
- Candidate compatibility is a conservative planning decision only. Task 08 does not verify an installed runtime, model artifact, real inference, performance, GPU offload, or provider-specific package identity.
- ARM64 and x86 are not included in the v0.1 runtime planning profile. They produce an explicit no-plan result rather than an inferred configuration.
- Removable storage may remain eligible when free-space evidence is sufficient, but the report warns that it must stay connected.

## Validation

From the repository root:

```powershell
cargo run -p gixgiz-contracts --example generate_bindings -- --check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

From `apps/desktop`:

```powershell
flutter pub get
flutter gen-l10n
flutter analyze
flutter test
flutter build windows
```
