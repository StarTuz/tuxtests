# Privileged Diagnostics And SMART Hardening Plan

This plan tracks the post-hybrid backend hardening work. The UI layers are now useful enough to expose diagnostic gaps, but the Rust backend remains the source of truth.

## Phase 1: Privileged Diagnostic Baseline

Status: started.

- Replace one-bit `smartctl -H` handling with structured `smartctl -x -j` parsing.
- Preserve raw smartctl exit codes and decode the smartctl bitmask into human-readable descriptions.
- Represent privilege failures as diagnostic data, not silent health failures.
- Keep CLI, TUI, and Tauri on the same backend path.

## Phase 2: SMART Interpretation Layer

Status: started.

- Add `DriveInfo.smart` as the structured per-drive SMART report.
- Add `TuxPayload.findings` for backend-derived diagnostic leads before AI analysis.
- Extract first-pass SMART counters for ATA and NVMe drives, including reallocated sectors, pending sectors, offline uncorrectable sectors, NVMe media errors, NVMe error-log entries, unsafe shutdowns, power-on hours, percentage used, and temperature.
- Treat SMART findings as evidence-backed leads. Avoid claiming root cause without trend history or corroborating kernel/device evidence.

## Phase 3: UI Surfacing

Status: started.

- Render backend-derived findings in the TUI diagnostics panel before raw kernel anomalies.
- Render selected-drive SMART details in the TUI drive details panel.
- Render backend-derived findings in the Tauri diagnostics card before raw kernel anomalies.
- Render selected-drive SMART details in the Tauri selected-drive card.

## Remaining Work

- Add richer SMART attribute coverage for USB bridge and SCSI/SAT edge cases.
- Add a privilege strategy that avoids repeated polkit prompt spam, likely by documenting sudo mode first and then adding an explicit elevated helper path.
- Add fixture-based regression tests for real-world smartctl JSON from NVMe, SATA HDD, SATA SSD, USB bridge, and access-denied outputs.
- Add trend/history support later so warnings can distinguish static old counters from actively worsening drives.

## Guardrails

- Do not let UI code invent SMART, PCIe, ASPM, or health facts.
- Do not suppress ASPM or SMART uncertainty. Surface uncertainty explicitly.
- Prefer primary device/kernel evidence over AI-only conclusions.
- Keep raw facts and explanatory findings separate so users can inspect both.
