# TuxTests CLI Documentation

This document provides a comprehensive guide to the command-line interface of TuxTests.

## 🛠 Commands and Flags

### Basic Analysis

- `--tui`: Launches the Ratatui terminal dashboard.
  - **Scope**: Uses the same backend payload collection and AI analysis flow as the standard CLI.
  - **Interaction**: `r` refresh, `b` full-bench refresh, `a` analyze, `c` edit config, `j/k` select drives, `tab` change scroll focus, `PgUp/PgDn` scroll active panel, `q` quit.
- `-a, --analyze`: Performs a standard, unprivileged hardware scan.
  - **Scope**: CPU, RAM, kernel, hostname, motherboard, block device topology, and USB connection speeds.
  - **Privileges**: None (does not prompt for password).
- `--full-bench`: Performs a deep, privileged diagnostic scan.
  - **Scope**: Includes everything in `--analyze`, plus S.M.A.R.T. health checks and 1GB buffered write benchmarks.
  - **Privileges**: Requires `pkexec` (Polkit) for drive health monitoring.
- `--dump-payload`: Emits the collected `TuxPayload` as pretty-printed JSON to `stdout` and skips AI analysis.
  - **Integration Note**: Progress and diagnostic messages go to `stderr`, so `stdout` remains machine-readable.
- `--mock <PATH>`: Loads a single `DriveInfo` fixture from disk and routes it through the analyzer or payload dumper.
  - **Use Case**: Regression testing and UI work without scanning live hardware.

### Configuration

- `--set-llm-provider <PROVIDER>`: Configures the preferred AI engine.
  - **Options**: `gemini` (Default), `ollama`.
- `--set-gemini-key <KEY>`: Securely stores your Google AI API key.
  - **Storage**: System Keyring (KWallet, GNOME Keyring, or Secret Service).
- `--set-ollama-url <URL>`: Configures the endpoint for local Ollama instances.
  - **Validation**: Must be a full `http://` or `https://` base URL.
  - **Default**: `http://127.0.0.1:11434`.
- `--set-ollama-model <MODEL>`: Specifically targets the local model (e.g., `gemma4:e4b`).
  - **Default**: `mistral`.
- `--print-config`: Emits the normalized runtime configuration as pretty-printed JSON to `stdout`.

---

## 🔒 Security and Permissions

TuxTests is built with a "Privacy First" and "Least Privilege" philosophy.

### Keyring Integration

API keys are never stored in plain text or environment variables. TuxTests uses the `keyring` crate to interface with your desktop's native secret manager. If a key is missing, the tool will provide a clear error message rather than attempting an unauthenticated request.

When running TuxTests through `sudo`, the tool can reuse the invoking user's config file, but the Gemini secret still lives in that user's desktop keyring. Root normally cannot read that keyring entry. For Gemini analysis, prefer running without `sudo`; for privileged scans, prefer `--full-bench`/Polkit or use Ollama for sudo-launched workflows. If you intentionally want a separate root Gemini key, set it explicitly with `sudo tuxtests --set-gemini-key "YOUR_KEY_HERE"`.

### Polkit Elevation

Privileged actions like reading S.M.A.R.T. data (`smartctl`) are handled via `pkexec`.

- TuxTests only requests elevation when strictly necessary (e.g., when the `--full-bench` flag is used).
- Individual commands are wrapped, meaning the tool never runs its entire logic as root.
- The Ratatui dashboard does not invoke interactive Polkit prompts during normal PCIe inspection. If richer PCIe visibility is needed, running TuxTests itself under `sudo` will expose more `lspci` detail than an unprivileged session.

---

## 🧪 Benchmarking Safety

The `--full-bench` command includes a synthetic write test to measure drive throughput.

- **Capacity Guard**: TuxTests will **never** perform a benchmark on a partition with 5GB or less free space, or with less than 10% free capacity.
- **Cleanup**: Benchmark files (`.tuxtests_bench.tmp`) are volatile and deleted immediately after the test completes.

---

## ❓ Troubleshooting

### "smartmontools missing or execution failed"

If you see this anomaly in your AI report, it means `smartctl` is not installed on your host system.

- **Fix**: Install it via your package manager (e.g., `sudo pacman -S smartmontools` or `sudo apt install smartmontools`).

### "Gemini API key natively blocked or missing"

This means the tool couldn't find a key in your system vault.

- **Fix**: Run `tuxtests --set-gemini-key "YOUR_KEY_HERE"` once to initialize the secure storage.

---

## 🔌 UI Contract

For frontend or automation integration, prefer these stable interfaces:

- `tuxtests --print-config`
  - Returns normalized config JSON on `stdout`.
- `tuxtests --dump-payload`
  - Returns the hardware scan payload JSON on `stdout`.
- `tuxtests --full-bench --dump-payload`
  - Returns the enriched payload including SMART and throughput fields on `stdout`.

Human-oriented status updates and troubleshooting messages are written to `stderr` so `stdout` can be consumed as structured data.

## 🖥 Terminal UI Notes

The Ratatui dashboard is the first hybrid UI layer built on top of the shared backend facade.

- Config edits in the dashboard are applied through the same backend validation and persistence path used by CLI flags.
- Long drive details, diagnostics, and analysis output are scrollable from within the dashboard.
- `--tui` is intended as an operator-facing workflow; for machine integration, keep using `--print-config` and `--dump-payload`.
