# Udev and Sysfs Logic Patterns

This document outlines the extraction of intelligent hardware topology inside Linux, directly leveraging `libudev` via Rust.

## 1. Traversing for USB Speeds via udev
Standard block device APIs (`sysinfo` or `lsblk`) fall short when attempting to diagnose if an external drive is bottlenecked by its physical connection port.

### Pattern: Finding the Parent USB Device
In `src/hardware/connection.rs`, the logic involves:
1. Creating a `udev::Enumerator` matching the block subsystem.
2. For each block device (`sdX`), traversing up the device tree (`device.parent()`) until a device from the `usb` subsystem is found.
3. Reading the `speed` sysfs attribute of the parent USB device.

### Speed Mappings:
- `"480"` -> USB 2.0 (High-Speed)
- `"5000"` -> USB 3.0/3.1 Gen 1 (SuperSpeed)
- `"10000"` -> USB 3.1 Gen 2 (SuperSpeed+)
- `"20000"` -> USB 3.2 Gen 2x2

## 2. Safety and Polkit Use
Iterating `udev` device trees is safe and does not require root. However, reading S.M.A.R.T. health through `smartctl` and creating the 1GB benchmark file do touch privileged or high-impact paths and are guarded carefully.

If we need deep `sysfs` capabilities beyond the standard unprivileged read, `/dev/sdX` access is brokered via Polkit rather than exposing raw `sudo` commands directly to the user.

## 3. Hybrid Log-Scraping RAG Strategy

Instead of complex vector embeddings, TuxTests utilizes **Identifier-Based Filtering** for local log retrieval.

### Pattern: Contextual Log Retrieval
The `src/ai/rag.rs` engine performs the following:
1. Identifies hardware handles (`/dev/sda`) and serial numbers (`XYZ123`).
2. Filters `dmesg` and `journalctl` output for those specific identifiers.
3. Extracts relevant kernel warnings (e.g., "I/O errors", "reset high-speed device") to augment the LLM context.

## 4. Mock Hardware Regression Testing

TuxTests uses a fixture-based testing harness to ensure compatibility across diverse Linux environments.

### Edge Case Fixtures:
- **"Slow Lane"**: NVMe drives behind USB adapters (reports high capacity but bottlenecked).
- **"Zombie" Drive**: Devices that appear in `lsblk` but return non-zero exit codes from `smartctl`.
- **LVM on LUKS**: Nested partitions where a device mapper node (`dm-0`) lives on an encrypted physical parent.

## 5. Type-Safe Hardware Modeling

TuxTests utilizes a centralized `src/models.rs` to define the hardware footprint.

### Pattern: Edge-Case Handling via Option<T>
Instead of brittle string parsing, the core engine deserializes hardware snapshots into strongly-typed structs.
- **Optional Attributes**: Fields like `serial`, `is_luks`, `parent`, `motherboard`, or `smartctl_exit_code` are wrapped in `Option<T>` where appropriate. This allows a single hardware model to represent anything from a standard SATA drive to a complex encrypted LVM mapper without type explosions.
- **PCIe Context**: Drives now carry a `pcie_path` collection when PCI devices are present in their topology. This gives downstream analysis a concrete place to inspect bridge/device BDFs, drivers, link speeds, widths, ASPM capability, observed ASPM state, and whether the reading came from unprivileged or privileged inspection. When probing fails, the payload preserves that failure in `aspm_probe_error` instead of silently flattening everything to `null`.
- **Serialization Determinism**: The system uses `std::collections::BTreeMap` for the benchmarks collection. This guarantees that drives are always presented to the LLM in a consistent, alphabetic order, preventing positional bias during analysis.

## 6. Benchmark Guardrails

The throughput path now enforces both absolute and relative free-space limits before writing a temporary benchmark file.

### Current Safety Rules:
- More than **5GB free** is required.
- At least **10% free capacity** is required.
- Benchmark diagnostics are emitted on `stderr` so JSON payload dumping can keep `stdout` machine-readable.

## 7. PCIe Recommendation Guardrails

TuxTests now collects the global Linux ASPM policy and per-drive PCIe path facts when they are visible on the host.

### Current Rule:
- ASPM-related remediation should be treated as conditional on observed payload facts, not as a generic recommendation.
- If unprivileged inspection cannot read PCIe link details for anomaly-linked devices, TuxTests retries those BDFs through privileged `lspci` paths and records either the successful source or the probe failure reason.

## 8. Secret Management via Keyring

To avoid leaking API keys in logs or process trees, TuxTests utilizes the `keyring` crate.

### Pattern: Native Credential Extraction
1. The tool identifies the target service (e.g., `"tuxtests"`).
2. It attempts to retrieve the secret from the system's native vault (KWallet, GNOME Keyring, or Secret Service API).
3. If the key is missing, the tool halts immediately with a user instruction, ensuring no unauthenticated or environment-leaking attempts are made.
