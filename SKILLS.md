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
Iterating `udev` device trees is safe and does not require root. However, triggering S.M.A.R.T. tests or creating the 1GB dummy file (benchmarks) does heavily dictate root privileges.

If we need deep `sysfs` capabilities beyond the standard unprivileged read, `/dev/sdX` access is brokered via Polkit rather than exposing raw `sudo` commands directly to the user.

## 3. Hybrid Log-Scraping RAG Strategy

Instead of complex vector embeddings, TuxTests utilizes **Identifier-Based Filtering** for local log retrieval.

### Pattern: Contextual Log Retrieval
The `src/ai/rag.rs` engine performs the following:
1. Identifies hardware handles (`/dev/sda`) and serial numbers (`XYZ123`).
2. Greps `dmesg` and `journalctl` for those specific identifiers.
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
- **Optional Attributes**: Fields like `is_luks`, `parent`, or `smartctl_exit_code` are wrapped in `Option<T>`. This allows a single `DriveInfo` struct to represent anything from a standard SATA drive to a complex encrypted LVM mapper without type explosions.
- **Serialization Determinism**: The system uses `std::collections::BTreeMap` for the benchmarks collection. This guarantees that drives are always presented to the LLM in a consistent, alphabetic order, preventing positional bias during analysis.

## 6. Secret Management via Keyring

To avoid leaking API keys in logs or process trees, TuxTests utilizes the `keyring` crate.

### Pattern: Native Credential Extraction
1. The tool identifies the target service (e.g., `"tuxtests"`).
2. It attempts to retrieve the secret from the system's native vault (KWallet, GNOME Keyring, or Secret Service API).
3. If the key is missing, the tool halts immediately with a user instruction, ensuring no unauthenticated or environment-leaking attempts are made.
