# TuxTests CLI Documentation

This document provides a comprehensive guide to the command-line interface of TuxTests.

## 🛠 Commands and Flags

### Basic Analysis

- `-a, --analyze`: Performs a standard, unprivileged hardware scan.
  - **Scope**: CPU, RAM, Block Device Topology, USB Connection Speeds.
  - **Privileges**: None (does not prompt for password).
- `--full-bench`: Performs a deep, privileged diagnostic scan.
  - **Scope**: Includes everything in `--analyze`, plus S.M.A.R.T. health checks and 1GB buffered write benchmarks.
  - **Privileges**: Requires `pkexec` (Polkit) for drive health monitoring.

### Configuration

- `--set-llm-provider <PROVIDER>`: Configures the preferred AI engine.
  - **Options**: `gemini` (Default), `ollama`.
- `--set-gemini-key <KEY>`: Securely stores your Google AI API key.
  - **Storage**: System Keyring (KWallet, GNOME Keyring, or Secret Service).
- `--set-ollama-url <URL>`: Configures the endpoint for local Ollama instances.
  - **Default**: `http://localhost:11434`.
- `--set-ollama-model <MODEL>`: Specifically targets the local model (e.g., `gemma4:e4b`).
  - **Default**: `mistral`.

---

## 🔒 Security and Permissions

TuxTests is built with a "Privacy First" and "Least Privilege" philosophy.

### Keyring Integration

API keys are never stored in plain text or environment variables. TuxTests uses the `keyring` crate to interface with your desktop's native secret manager. If a key is missing, the tool will provide a clear error message rather than attempting an unauthenticated request.

### Polkit Elevation

Privileged actions like reading S.M.A.R.T. data (`smartctl`) are handled via `pkexec`.

- TuxTests only requests elevation when strictly necessary (e.g., when the `--full-bench` flag is used).
- Individual commands are wrapped, meaning the tool never runs its entire logic as root.

---

## 🧪 Benchmarking Safety

The `--full-bench` command includes a synthetic write test to measure drive throughput.

- **Capacity Guard**: TuxTests will **never** perform a benchmark on a partition with less than 5GB or 10% free space.
- **Cleanup**: Benchmark files (`.tuxtests_bench.tmp`) are volatile and deleted immediately after the test completes.

---

## ❓ Troubleshooting

### "smartmontools missing or execution failed"

If you see this anomaly in your AI report, it means `smartctl` is not installed on your host system.

- **Fix**: Install it via your package manager (e.g., `sudo pacman -S smartmontools` or `sudo apt install smartmontools`).

### "Gemini API key natively blocked or missing"

This means the tool couldn't find a key in your system vault.

- **Fix**: Run `tuxtests --set-gemini-key "YOUR_KEY_HERE"` once to initialize the secure storage.
