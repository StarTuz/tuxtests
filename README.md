# TuxTests: Linux Hardware & Drive Intelligence Tool

TuxTests is a high-performance, memory-safe Linux hardware diagnostic and suggestion tool built in Rust. It utilizes modern AI (Gemini/Ollama) to identify system bottlenecks and provide actionable upgrade advice.

The current committed Rust CLI/backend is the source of truth. Ratatui and Tauri interfaces are layered on top of this backend contract rather than replacing or re-implementing hardware collection logic.

## 🚀 Features

- **Core Hardware Investigation**: CPU, RAM, kernel, hostname, motherboard, and block device identification using `sysinfo`, `lsblk`, and Linux system files.
- **Connection Intelligence**: [ACTIVE] Deep `udev` tree traversal to identify physical port bottlenecks (e.g., fast SSDs on legacy USB 2.0 ports).
- **S.M.A.R.T. Integration**: [ACTIVE] Drive health monitoring via `smartctl` with Polkit elevation.
- **Non-Destructive Benchmarking**: [ACTIVE] Buffered read/write testing with 5GB and 10% free-space safety checks.
- **AI-Driven Analysis**: [ACTIVE] Integration with Google Gemini (Cloud) and Ollama (Local) for bottleneck identification.
- **Hybrid Log-Scraping RAG**: [ACTIVE] Context-aware log retrieval using identifier-based filtering over `dmesg` and `journalctl`.

## 🛠 Tech Stack

- **Language**: Rust
- **CLI**: `clap`
- **Terminal UI**: `ratatui`
- **Graphical UI**: `tauri` static frontend shell over the Rust backend
- **Async Runtime**: `tokio`
- **Hardware APIs**: `libudev`, `sysinfo`, `lsblk`
- **Serialization**: `serde`, `serde_json`
- **Configuration**: `--set-ollama-model <MODEL>` and `--set-ollama-url <URL>` for local Ollama targeting.
- **Storage**: Persistent configuration in `~/.config/tuxtests/config.toml` with local safety fallbacks for read-only environments.

---

## 🔒 Security and Permissions

- **Security**: `keyring` (Secure API key storage), `Polkit` (Privilege elevation)
- **Networking**: `reqwest` (Async HTTP with 60s safeguards)

Gemini keys are stored in the current desktop user's native keyring. A sudo-launched TuxTests session can reuse the invoking user's config file, but root normally cannot read that user's keyring secret. Prefer non-sudo Gemini analysis, Polkit-backed `--full-bench`, or Ollama for sudo-launched workflows.

## ⌨️ Usage

TuxTests is currently in a functional MVP state. For detailed flag documentation, see [CLI.md](CLI.md).

```bash
# Launch the Ratatui terminal dashboard
tuxtests --tui

# Set your Gemini API key in the secure keyring
tuxtests --set-gemini-key "your_api_key_here"

# Run a standard unprivileged analysis
tuxtests --analyze

# Run a privileged scan including structured SMART health and benchmarks
tuxtests --full-bench

# Point at a non-default local or remote Ollama endpoint
tuxtests --set-ollama-url "http://127.0.0.1:11434"

# Emit machine-readable payload JSON for UI or automation
tuxtests --dump-payload

# Print normalized runtime config as JSON
tuxtests --print-config
```

### Ratatui Dashboard

`tuxtests --tui` launches the first hybrid-interface slice on top of the shared backend facade. It does not reimplement hardware logic in the UI layer.

- Uses the same payload collection and AI analysis path as the classic CLI.
- Supports standard refresh with `r`, full-bench refresh with `b`, and AI analysis with `a`.
- Lets you move between drives with `j/k` or the arrow keys.
- Includes backend-driven config editing with `c`.
- Supports scroll focus cycling with `tab` and panel scrolling with `PgUp` / `PgDn`.

### Tauri GUI

The first Tauri slice lives in `src-tauri/` and intentionally stays thin:

- `get_config` calls the shared Rust config facade.
- `update_config` applies provider/model/url edits through the shared Rust validation path.
- `get_payload` calls the shared Rust hardware payload collector.
- `get_payload(full_bench=true)` uses the same deeper structured SMART/benchmark path as the CLI/TUI.
- `analyze_payload` calls the quiet shared AI analysis path.
- The frontend only renders system, drive, diagnostic, and analysis data returned by the backend.

This GUI shell is an early hybrid milestone, not a replacement for the CLI/TUI.

Developer commands:

```bash
# Verify the Tauri Rust command layer
npm run tauri:check

# Launch the graphical shell during development
npm run tauri:dev

# Fallback for Wayland/WebKitGTK/NVIDIA protocol issues
npm run tauri:dev:x11
```

## 🏗 Project Structure

- `src/hardware/`: Logic for system, storage, and connection discovery.
- `src/models.rs`: Core Rust data structures (`TuxPayload`, `DriveInfo`) shared across discovery and AI modules.
- `src/bench/`: Benchmarking and drive health logic.
- `src/ai/`: LLM integration, configuration management, and the RAG log-scraping engine.
- `src/ui/`: Ratatui terminal dashboard.
- `src-tauri/`: Tauri graphical shell that invokes the shared Rust backend.
- `tests/fixtures/`: Mocked hardware scenarios for regression testing.

## 🧪 Testing & CI/CD

TuxTests emphasizes reliability through:
- **Regression Testing**: A suite of mocked hardware topographies (LVM on LUKS, USB/NVMe slow lanes, Zombie drives).
- **GitHub Actions**: Automated pipelines with `libudev-dev` support for `clippy`, `fmt`, and hardware-fixture verification.

## 📦 Documentation

- [CLI.md](CLI.md): Detailed Command Line Interface guide.
- [GEMINI.md](GEMINI.md): AI Pipeline and JSON Schema definitions (Dynamic v1beta mapping).
- [SKILLS.md](SKILLS.md): Technical details on `udev` and RAG strategies.
- [docs/UI_CONTRACT.md](docs/UI_CONTRACT.md): Stable frontend and automation integration contract.
- [docs/HYBRID_ARCHITECTURE_PLAN.md](docs/HYBRID_ARCHITECTURE_PLAN.md): Grounded plan for Ratatui/Tauri on top of the validated backend.
