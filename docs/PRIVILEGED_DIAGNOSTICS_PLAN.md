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

Status: implemented.

- Render backend-derived findings in the TUI diagnostics panel before raw kernel anomalies.
- Render selected-drive SMART details in the TUI drive details panel.
- Render backend-derived findings in the Tauri diagnostics card before raw kernel anomalies.
- Render selected-drive SMART details in the Tauri selected-drive card.

## Phase 4: Privilege Noise Reduction

Status: implemented.

- Use direct `smartctl -x -j` when TuxTests is already running with effective UID 0.
- Use `pkexec smartctl -x -j` only for unprivileged desktop-user sessions.
- Skip SMART probes for virtual or mapped block devices such as zram, loop, RAM disks, and dm nodes.
- Represent skipped devices as informational backend findings rather than warnings.

## Phase 5: Regression Coverage

Status: implemented.

- Cover direct-root detection helper parsing for `/proc/self/status`.
- Cover virtual block-device SMART skip decisions.
- Cover informational finding classification for SMART-not-applicable devices.

## Phase 6: SMART Advisory Findings

Status: implemented.

- Add threshold-based advisory findings for high SMART temperature.
- Add threshold-based advisory findings for elevated NVMe percentage-used endurance values.
- Add advisory findings for notable NVMe unsafe-shutdown counts.
- Keep these as planning/triage guidance rather than definitive failure claims.

## Phase 7: SMART Parser Coverage

Status: implemented.

- Parse common ATA SMART attributes by ID as well as by canonical attribute name.
- Parse ATA raw string values when raw numeric values are not present.
- Use ATA SMART attributes as fallbacks for temperature, power-on hours, and power-cycle counts.
- Map SCSI grown defect counts into the existing degradation counter field.

## Phase 8: SMART Probe Status

Status: started.

- Add a structured SMART probe status to distinguish available, not-applicable, access-denied, missing-tool, execution-failed, and parse-failed states.
- Use the structured status for backend finding classification instead of inferring from limitation text.
- Surface the probe status in TUI and Tauri SMART detail views.

## Remaining Work

- Add richer SMART attribute coverage for USB bridge and SCSI/SAT edge cases.
- Continue refining privilege strategy to avoid repeated prompts across multiple real disks, likely by documenting sudo mode first and then adding an explicit elevated helper path.
- Add fixture-based regression tests for real-world smartctl JSON from NVMe, SATA HDD, SATA SSD, USB bridge, and access-denied outputs.
- Add trend/history support later so warnings can distinguish static old counters from actively worsening drives.

## Guardrails

- Do not let UI code invent SMART, PCIe, ASPM, or health facts.
- Do not suppress ASPM or SMART uncertainty. Surface uncertainty explicitly.
- Prefer primary device/kernel evidence over AI-only conclusions.
- Keep raw facts and explanatory findings separate so users can inspect both.
