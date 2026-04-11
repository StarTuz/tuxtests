# TuxTests UI Contract

This document defines the supported machine-readable interfaces a UI or automation layer should rely on.

## Stable Entry Points

### `tuxtests --print-config`

Purpose: Return the normalized runtime configuration.

Behavior:
- Writes pretty-printed JSON to `stdout`
- Exits without running hardware scans or AI analysis
- Uses the same normalization logic as the live runtime

Example shape:

```json
{
  "provider": "ollama",
  "ollama_model": "mistral",
  "ollama_url": "http://127.0.0.1:11434"
}
```

### `tuxtests --dump-payload`

Purpose: Return the collected hardware scan payload without AI analysis.

Behavior:
- Writes pretty-printed `TuxPayload` JSON to `stdout`
- Writes progress, warnings, and diagnostics to `stderr`
- Performs the same unprivileged scan as `--analyze`

### `tuxtests --full-bench --dump-payload`

Purpose: Return the enriched hardware payload including SMART and benchmark results.

Behavior:
- Writes pretty-printed `TuxPayload` JSON to `stdout`
- Writes progress, warnings, and diagnostics to `stderr`
- Runs the same backend path used by the TUI and Tauri `full_bench=true` command
- Includes structured SMART reports from `smartctl -x -j` when privileged access succeeds
- Preserves SMART exit codes, exit-status descriptions, benchmark results, and privilege-related anomalies when available
- Adds cautious backend-derived `findings` for SMART availability, failed SMART status, non-zero SMART counters, and PCIe error-containment leads

## Output Rules

- Treat `stdout` as the data channel for `--print-config` and `--dump-payload`.
- Treat `stderr` as the diagnostics channel for progress messages, warnings, and errors.
- Do not parse human-readable AI markdown for UI state.
- Prefer rendering from `TuxPayload` JSON and using AI output only as an optional explanatory layer.

## Supported Schema Sources

- `TuxPayload`: [src/models.rs](/home/startux/Code/tuxtests/src/models.rs)
- CLI behavior: [src/main.rs](/home/startux/Code/tuxtests/src/main.rs)
- AI payload example: [GEMINI.md](/home/startux/Code/tuxtests/GEMINI.md)

Important schema fields for diagnostics:

- `DriveInfo.smart`: optional structured SMART report; absent when a deep SMART pass has not been run
- `DriveInfo.smartctl_exit_code`: raw smartctl exit code when available
- `TuxPayload.findings`: backend-derived diagnostic leads intended for UI rendering before any AI explanation
- `TuxPayload.kernel_anomalies`: raw anomaly strings preserved for context and AI prompting

## In-Process UI Facade

Ratatui and Tauri should prefer the shared Rust facade in [src/engine.rs](/home/startux/Code/tuxtests/src/engine.rs) rather than shelling out to the CLI:

- `load_config()` / `config_json()`
- `apply_config_update(...)`
- `collect_payload(full_bench)`
- `analyze_payload(...)` or `analyze_payload_quiet(...)`

The Tauri command layer in [src-tauri/src/commands.rs](/home/startux/Code/tuxtests/src-tauri/src/commands.rs) is intentionally thin and should remain a pass-through over that facade.

## Notes

- The AI analysis path remains human-oriented and returns Markdown.
- Frontends should treat provider errors, missing keyring state, Polkit denials, and benchmark skips as diagnostic events surfaced on `stderr`.
