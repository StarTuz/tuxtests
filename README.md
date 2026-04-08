# TuxTests: Linux Hardware & Drive Intelligence Tool

TuxTests is a high-performance, memory-safe Linux hardware diagnostic and suggestion tool built in Rust. It utilizes modern AI (Gemini/Ollama) to identify system bottlenecks and provide actionable upgrade advice.

## 🚀 Features

- **Core Hardware Investigation**: CPU, RAM, and Motherboard identification using `sysinfo`.
- **Connection Intelligence**: [ACTIVE] Deep `udev` tree traversal to identify physical port bottlenecks (e.g., fast SSDs on legacy USB 2.0 ports).
- **S.M.A.R.T. Integration**: [ACTIVE] Drive health monitoring via `smartctl` with Polkit elevation.
- **Non-Destructive Benchmarking**: [ACTIVE] Buffered read/write testing with 5GB safety checks.
- **AI-Driven Analysis**: [ACTIVE] Integration with Google Gemini (Cloud) and Ollama (Local) for bottleneck identification.
- **Hybrid Log-Scraping RAG**: [ACTIVE] Context-aware log retrieval using identifier-based filtering over `dmesg` and `journalctl`.

## 🛠 Tech Stack

- **Language**: Rust
- **CLI**: `clap`
- **Async Runtime**: `tokio`
- **Hardware APIs**: `libudev`, `sysinfo`, `procfs`
- **Serialization**: `serde`, `serde_json`
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
- **GitHub Actions**: Automated pipelines for `clippy`, `fmt`, and hardware-fixture verification.

## 📦 Documentation

- [CLI.md](CLI.md): Detailed Command Line Interface guide.
- [GEMINI.md](GEMINI.md): AI Pipeline and JSON Schema definitions.
- [SKILLS.md](SKILLS.md): Technical details on `udev` and RAG strategies.
