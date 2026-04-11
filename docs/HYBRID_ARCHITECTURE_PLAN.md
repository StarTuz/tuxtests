# TuxTests Hybrid Architecture Plan

This document defines the intended path from the current validated Rust CLI/backend to a hybrid application with:

- a terminal UI built with Ratatui
- a graphical UI built with Tauri

The key rule is simple:

The existing Rust backend remains the source of truth for discovery, benchmarking, AI routing, schema, and diagnostics. UI layers consume that backend; they do not reimplement it.

## Goals

1. Preserve the validated hardware collector and analyzer behavior already shipped in the CLI.
2. Reuse the existing `TuxPayload` and config contract for both UI surfaces.
3. Keep machine-readable interfaces stable while adding richer presentation.
4. Avoid speculative or placeholder hardware logic in UI-facing code.

## Source Of Truth

These backend pieces should continue to own the core behavior:

- [src/main.rs](/home/startux/Code/tuxtests/src/main.rs): CLI entry points and machine-readable modes
- [src/models.rs](/home/startux/Code/tuxtests/src/models.rs): canonical payload schema
- [src/hardware](/home/startux/Code/tuxtests/src/hardware): hardware discovery and PCIe enrichment
- [src/bench](/home/startux/Code/tuxtests/src/bench): SMART and benchmark logic
- [src/ai](/home/startux/Code/tuxtests/src/ai): provider routing, prompts, config, and RAG behavior
- [docs/UI_CONTRACT.md](/home/startux/Code/tuxtests/docs/UI_CONTRACT.md): supported integration contract
- [docs/PRIVILEGED_DIAGNOSTICS_PLAN.md](/home/startux/Code/tuxtests/docs/PRIVILEGED_DIAGNOSTICS_PLAN.md): privileged SMART and diagnostic hardening plan

## Layering Strategy

### Phase A: Extract a reusable backend facade

Create a small internal Rust API that wraps the already-proven CLI logic:

- `collect_payload(full_bench: bool) -> TuxPayload`
- `print_config_json() -> AppConfig`
- `run_analysis(payload: &TuxPayload) -> AnalysisResult`

This should be a thin refactor of the current code, not a rewrite.

### Phase B: Ratatui terminal interface

Build the terminal dashboard as a presentation layer over the same facade:

- start with read-only payload rendering
- add explicit action keys for refresh, analyze, and bench
- keep AI output separate from payload facts
- preserve `stderr`/diagnostic semantics where practical

The Ratatui app should never fabricate PCIe, storage, or ASPM state. It should only render backend results.

### Phase C: Tauri graphical interface

Build the Tauri shell as a second presentation layer over the same backend facade:

- expose `collect_payload`, config reads/writes, and `run_analysis` as Tauri commands
- serialize the same `TuxPayload` used by the CLI
- surface diagnostics distinctly from data
- keep frontend logic focused on rendering, filtering, and user workflow

Initial Phase C slice:

- `src-tauri/src/commands.rs` exposes only thin backend calls: `get_config`, `update_config`, `get_payload`, and `analyze_payload`.
- `src-tauri/frontend/` is a static frontend with no hardware mocks or separate schema.
- Future GUI features should extend these commands or the shared backend facade, not create UI-local collection code.

## Non-Goals

These are explicitly out of scope for the first hybrid pass:

- rewriting hardware discovery inside a separate UI-specific backend
- inventing placeholder PCIe or ASPM facts for demonstration purposes
- changing schema shape independently in one UI surface
- letting UI code become the owner of AI/provider business logic

## Guardrails

1. No placeholder hardware values in production paths.
2. No UI-specific schema forks.
3. No replacement of validated collector logic without tests proving parity.
4. Every new UI action must map to an existing backend capability or a deliberately added backend API.

## Migration Rules

These rules apply to every step of the hybrid migration:

1. Move code by extraction or reuse before rewriting behavior.
2. Keep hardware collection in Rust backend modules, not in UI adapters.
3. Treat `TuxPayload` as the canonical shared contract until a deliberate schema migration is approved.
4. Do not mark documentation, installer text, or versions as hybrid-ready until both UI layers build and consume real backend data.
5. If a UI prototype needs mock data for layout work, keep that data in clearly labeled frontend-only fixtures and never mix it into production collection paths.

## Anti-Patterns

The following patterns are explicitly forbidden in hybrid work:

- hardcoding PCIe BDFs, ASPM states, storage devices, or other machine facts in runtime code
- creating a second backend tree that drifts from the validated collector/analyzer
- updating README, installer scripts, or crate versions to advertise UI features before they build and run
- replacing proven diagnostics with simplified placeholders "for now" without clear isolation
- deleting or sidelining the CLI before the shared backend facade is in place

## Implementation Checklist

Before opening a hybrid UI PR, confirm all of the following:

- the existing CLI behavior still passes `fmt`, `test`, `clippy`, and release build checks
- the UI path calls into shared backend code rather than duplicating collector logic
- no production code contains mock hardware values
- machine-readable outputs still match [docs/UI_CONTRACT.md](/home/startux/Code/tuxtests/docs/UI_CONTRACT.md)
- documentation describes only features that are actually present in the branch
- any new UI-specific dependencies are scoped to the UI layer and do not change backend behavior by accident

## Suggested Build Order

1. Extract backend facade from the current CLI implementation.
2. Add unit/integration coverage to prove parity with existing CLI modes.
3. Build a minimal Ratatui shell that renders `TuxPayload`.
4. Build a minimal Tauri shell that invokes the same backend facade.
5. Add richer UX only after backend parity is verified.

## Definition Of Done For Hybrid Work

A hybrid milestone is only complete when:

- the CLI still works
- the backend contract still matches [docs/UI_CONTRACT.md](/home/startux/Code/tuxtests/docs/UI_CONTRACT.md)
- the Ratatui layer renders real backend data
- the Tauri layer renders real backend data
- no UI layer contains fabricated hardware or PCIe logic
- tests cover the backend behaviors both UIs rely on
