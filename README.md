# TuxTests: Linux Hardware & Drive Intelligence Tool

TuxTests is a high-performance, memory-safe Linux hardware diagnostic and suggestion tool built in Rust. It utilizes modern AI (Gemini/Ollama) to identify system bottlenecks and provide actionable upgrade advice.

The current committed Rust CLI/backend is the source of truth. Future Ratatui and Tauri interfaces should be layered on top of this backend contract rather than replacing or re-implementing hardware collection logic.

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
- **Async Runtime**: `tokio`
- **Hardware APIs**: `libudev`, `sysinfo`, `lsblk`
- **Serialization**: `serde`, `serde_json`
- **Configuration**: `--set-ollama-model <MODEL>` and `--set-ollama-url <URL>` for local Ollama targeting.
- **Storage**: Persistent configuration in `~/.config/tuxtests/config.toml` with local safety fallbacks for read-only environments.

---

## 🔒 Security and Permissions

- **Security**: `keyring` (Secure API key storage), `Polkit` (Privilege elevation)
- **Networking**: `reqwest` (Async HTTP with 60s safeguards)

## ⌨️ Usage

TuxTests is currently in a functional MVP state. For detailed flag documentation, see [CLI.md](CLI.md).

```bash
# Set your Gemini API key in the secure keyring
tuxtests --set-gemini-key "your_api_key_here"

# Run a standard unprivileged analysis
tuxtests --analyze

# Run a privileged scan including SMART health and benchmarks
tuxtests --full-bench

# Point at a non-default local or remote Ollama endpoint
tuxtests --set-ollama-url "http://127.0.0.1:11434"

# Emit machine-readable payload JSON for UI or automation
tuxtests --dump-payload

# Print normalized runtime config as JSON
tuxtests --print-config
```

## 🏗 Project Structure

- `src/hardware/`: Logic for system, storage, and connection discovery.
- `src/models.rs`: Core Rust data structures (`TuxPayload`, `DriveInfo`) shared across discovery and AI modules.
- `src/bench/`: Benchmarking and drive health logic.
- `src/ai/`: LLM integration, configuration management, and the RAG log-scraping engine.
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
