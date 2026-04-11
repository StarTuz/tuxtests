const invoke = window.__TAURI__.core.invoke;

const state = {
  config: null,
  payload: null,
  selectedDrive: 0,
  analysis: null,
  analysisMode: "rendered",
};

const el = {
  status: document.querySelector("#status"),
  summary: document.querySelector("#summary"),
  system: document.querySelector("#system"),
  drives: document.querySelector("#drives"),
  driveCount: document.querySelector("#drive-count"),
  details: document.querySelector("#drive-details"),
  diagnostics: document.querySelector("#diagnostics"),
  analysis: document.querySelector("#analysis"),
  analysisRendered: document.querySelector("#analysis-rendered-view"),
  analysisRenderedToggle: document.querySelector("#analysis-rendered"),
  analysisRawToggle: document.querySelector("#analysis-raw"),
  refresh: document.querySelector("#refresh"),
  fullBench: document.querySelector("#full-bench"),
  analyze: document.querySelector("#analyze"),
  configForm: document.querySelector("#config-form"),
  provider: document.querySelector("#provider"),
  ollamaModel: document.querySelector("#ollama-model"),
  ollamaUrl: document.querySelector("#ollama-url"),
};

function setStatus(message) {
  el.status.textContent = message;
}

function renderSystem() {
  if (!state.payload) {
    el.system.innerHTML = "<dt>Status</dt><dd>Waiting for payload</dd>";
    return;
  }

  const system = state.payload.system;
  const prettyName = system.os_release?.PRETTY_NAME ?? "Unknown Linux";
  const provider = state.config?.provider ?? "unknown";
  const model = state.config?.ollama_model ?? "unknown";

  el.system.innerHTML = `
    <dt>Host</dt><dd>${escapeHtml(system.hostname)}</dd>
    <dt>OS</dt><dd>${escapeHtml(prettyName)}</dd>
    <dt>Kernel</dt><dd>${escapeHtml(system.kernel_version)}</dd>
    <dt>CPU</dt><dd>${escapeHtml(system.cpu)}</dd>
    <dt>RAM</dt><dd>${system.ram_gb} GB</dd>
    <dt>ASPM Policy</dt><dd>${escapeHtml(system.pcie_aspm_policy ?? "unknown")}</dd>
    <dt>AI Provider</dt><dd>${escapeHtml(provider)} (${escapeHtml(model)})</dd>
  `;
}

function renderSummary() {
  if (!state.payload) {
    el.summary.innerHTML = "";
    return;
  }

  const drives = state.payload.drives ?? [];
  const usbCount = drives.filter((drive) => drive.connection.toLowerCase().includes("usb")).length;
  const warningCount = drives.filter((drive) => !drive.health_ok).length;
  const anomalyCount = state.payload.kernel_anomalies?.length ?? 0;
  const uniqueAnomalyCount = groupCounts(state.payload.kernel_anomalies ?? []).length;
  const benchmarkCount = Object.keys(state.payload.benchmarks ?? {}).length;
  const findingCount = state.payload.findings?.length ?? 0;

  el.summary.innerHTML = `
    <span>${drives.length} drives</span>
    <span>${usbCount} USB</span>
    <span>${warningCount} warnings</span>
    <span>${findingCount} findings</span>
    <span>${uniqueAnomalyCount}/${anomalyCount} anomaly types</span>
    <span>${benchmarkCount} benchmarks</span>
  `;
}

function renderConfigForm() {
  if (!state.config) {
    return;
  }

  el.provider.value = state.config.provider;
  el.ollamaModel.value = state.config.ollama_model;
  el.ollamaUrl.value = state.config.ollama_url;
}

function renderDrives() {
  const drives = state.payload?.drives ?? [];
  el.driveCount.textContent = `${drives.length} drive${drives.length === 1 ? "" : "s"}`;

  el.drives.innerHTML = drives
    .map((drive, index) => {
      const selected = index === state.selectedDrive ? " selected" : "";
      return `
        <button class="drive-row${selected}" data-index="${index}">
          <span>${escapeHtml(drive.name)}</span>
          <span>${escapeHtml(drive.connection)}</span>
          <span>${drive.capacity_gb} GB</span>
          <span>${drive.usage_percent}%</span>
          <span>${benchmarkForDrive(drive.name)}</span>
        </button>
      `;
    })
    .join("");

  for (const button of el.drives.querySelectorAll(".drive-row")) {
    button.addEventListener("click", () => {
      state.selectedDrive = Number(button.dataset.index);
      renderDrives();
      renderDetails();
    });
  }
}

function renderDetails() {
  const drive = state.payload?.drives?.[state.selectedDrive];
  if (!drive) {
    el.details.textContent = "Select a drive to inspect its backend payload.";
    return;
  }

  const mounts = drive.active_mountpoints?.length ? drive.active_mountpoints.join(", ") : "none";
  const topology = drive.topology?.length
    ? drive.topology
        .map((node) => `- L${node.level} ${node.subsystem} ${node.sysname}`)
        .join("\n")
    : "No topology nodes in payload.";
  const pciePath = drive.pcie_path?.length
    ? drive.pcie_path
        .map((path) => {
          const speed = path.current_link_speed ?? "?";
          const width = path.current_link_width ? `x${path.current_link_width}` : "x?";
          const aspm = path.aspm ?? "ASPM unknown";
          const capability = path.aspm_capability ? ` capability=${path.aspm_capability}` : "";
          const probe = path.aspm_probe_error ? ` probe=${path.aspm_probe_error}` : "";
          return `- ${path.bdf} ${speed} ${width}: ${aspm}${capability}${probe}`;
        })
        .join("\n")
    : "No PCIe path in payload.";
  const benchmark = benchmarkForDrive(drive.name);
  const smart = renderSmartDetails(drive.smart);

  el.details.innerHTML = `
    <dl>
      <dt>Name</dt><dd>${escapeHtml(drive.name)}</dd>
      <dt>Type</dt><dd>${escapeHtml(drive.type)}</dd>
      <dt>Connection</dt><dd>${escapeHtml(drive.connection)}</dd>
      <dt>Mounts</dt><dd>${escapeHtml(mounts)}</dd>
      <dt>Health</dt><dd>${drive.health_ok ? "OK" : "Needs attention"}</dd>
      <dt>Serial</dt><dd>${escapeHtml(drive.serial ?? "unknown")}</dd>
      <dt>Benchmark</dt><dd>${escapeHtml(benchmark)}</dd>
    </dl>
    <h3>SMART</h3>
    ${smart}
    <h3>Topology</h3>
    <pre>${escapeHtml(topology)}</pre>
    <h3>PCIe Path</h3>
    <pre>${escapeHtml(pciePath)}</pre>
  `;
}

function renderDiagnostics() {
  const anomalies = state.payload?.kernel_anomalies ?? [];
  const findings = state.payload?.findings ?? [];
  const sections = [];

  if (findings.length) {
    sections.push(
      `<div class="diagnostic-section">
        <h3>Backend Findings</h3>
        ${findings.map(renderFinding).join("")}
      </div>`,
    );
  }

  if (anomalies.length) {
    sections.push(
      `<div class="diagnostic-section">
        <h3>Kernel Anomalies</h3>
        ${groupCounts(anomalies)
          .map(
            ({ value, count }) => `
              <article class="diagnostic-item">
                <strong>${count}x</strong>
                <span>${escapeHtml(value)}</span>
              </article>
            `,
          )
          .join("")}
      </div>`,
    );
  }

  if (!sections.length) {
    el.diagnostics.innerHTML = "<p>No findings or kernel anomalies in payload.</p>";
    return;
  }

  el.diagnostics.innerHTML = sections.join("");
}

function renderAnalysis(markdown) {
  const text = markdown ?? "Run analysis after a payload refresh.";
  state.analysis = text;
  el.analysis.textContent = text;
  el.analysisRendered.innerHTML = markdownToHtml(text);
  el.analysisRendered.classList.toggle("hidden", state.analysisMode !== "rendered");
  el.analysis.classList.toggle("hidden", state.analysisMode !== "raw");
  el.analysisRenderedToggle.classList.toggle("active", state.analysisMode === "rendered");
  el.analysisRawToggle.classList.toggle("active", state.analysisMode === "raw");
}

async function refreshPayload(fullBench = false) {
  setBusy(true);
  setStatus(fullBench ? "Collecting full-bench backend payload..." : "Collecting backend payload...");
  try {
    state.config = await invoke("get_config");
    renderConfigForm();
    state.payload = await invoke("get_payload", { fullBench });
    state.selectedDrive = Math.min(state.selectedDrive, Math.max(0, state.payload.drives.length - 1));
    renderSummary();
    renderSystem();
    renderDrives();
    renderDetails();
    renderDiagnostics();
    setStatus(
      `Loaded ${state.payload.drives.length} drives from the shared Rust backend${fullBench ? " with full-bench data" : ""}.`,
    );
  } catch (error) {
    setStatus(`Payload refresh failed: ${error}`);
  } finally {
    setBusy(false);
  }
}

async function saveConfig(event) {
  event.preventDefault();

  setBusy(true);
  setStatus("Saving AI configuration through the shared Rust backend...");
  try {
    state.config = await invoke("update_config", {
      provider: el.provider.value,
      ollamaModel: el.ollamaModel.value,
      ollamaUrl: el.ollamaUrl.value,
    });
    renderConfigForm();
    renderSystem();
    setStatus(`Saved AI config: ${state.config.provider} / ${state.config.ollama_model}.`);
  } catch (error) {
    setStatus(`Config save failed: ${error}`);
  } finally {
    setBusy(false);
  }
}

async function analyzePayload() {
  if (!state.payload) {
    setStatus("Refresh the payload before running analysis.");
    return;
  }

  setBusy(true);
  setStatus("Running AI analysis through the shared Rust backend...");
  renderAnalysis("AI analysis in progress...");
  try {
    const analysis = await invoke("analyze_payload", { payload: state.payload });
    renderAnalysis(analysis);
    setStatus("AI analysis completed.");
  } catch (error) {
    renderAnalysis(`Analysis failed:\n${error}`);
    setStatus("AI analysis failed.");
  } finally {
    setBusy(false);
  }
}

function setBusy(isBusy) {
  el.refresh.disabled = isBusy;
  el.fullBench.disabled = isBusy;
  el.analyze.disabled = isBusy;
  el.provider.disabled = isBusy;
  el.ollamaModel.disabled = isBusy;
  el.ollamaUrl.disabled = isBusy;
  el.configForm.querySelector("button").disabled = isBusy;
}

function benchmarkForDrive(name) {
  const result = state.payload?.benchmarks?.[name];
  return result ? `${result.write_mb_s} MB/s` : "not run";
}

function renderSmartDetails(smart) {
  if (!smart) {
    return `<p class="muted">No structured SMART report yet. Run Full Bench to collect one.</p>`;
  }

  const rows = [
    ["Status", smart.status ?? "unknown"],
    ["Available", smart.available ? "yes" : "no"],
    ["Passed", smart.passed === null || smart.passed === undefined ? "unknown" : smart.passed ? "yes" : "no"],
    ["Transport", smart.transport ?? "unknown"],
    ["Exit code", smart.smartctl_exit_code ?? "unknown"],
    ["Model", smart.model ?? "unknown"],
    ["Temperature", formatOptionalMetric(smart.temperature_celsius, " C")],
    ["Power-on hours", smart.power_on_hours ?? "unknown"],
    ["Percentage used", formatOptionalMetric(smart.percentage_used, "%")],
    ["Reallocated sectors", smart.reallocated_sectors ?? "unknown"],
    ["Pending sectors", smart.current_pending_sectors ?? "unknown"],
    ["Offline uncorrectable", smart.offline_uncorrectable ?? "unknown"],
    ["NVMe media errors", smart.media_errors ?? "unknown"],
    ["NVMe error-log entries", smart.num_err_log_entries ?? "unknown"],
  ];

  const status = smart.exit_status_description?.length
    ? `<p><strong>Exit status:</strong> ${escapeHtml(smart.exit_status_description.join("; "))}</p>`
    : "";
  const limitations = smart.limitations?.length
    ? `<p><strong>Limitations:</strong> ${escapeHtml(smart.limitations.join("; "))}</p>`
    : "";

  return `
    <dl class="smart-grid">
      ${rows
        .map(([label, value]) => `<dt>${escapeHtml(label)}</dt><dd>${escapeHtml(value)}</dd>`)
        .join("")}
    </dl>
    ${status}
    ${limitations}
  `;
}

function renderFinding(finding) {
  const severity = finding.severity ?? "notice";
  const category = finding.category ?? "diagnostic";
  const drive = finding.drive ? ` · ${finding.drive}` : "";
  const action = finding.recommended_action
    ? `<p><strong>Action:</strong> ${escapeHtml(finding.recommended_action)}</p>`
    : "";

  return `
    <article class="finding finding-${escapeHtml(severity)}">
      <div class="finding-meta">${escapeHtml(severity)} · ${escapeHtml(category)}${escapeHtml(drive)}</div>
      <strong>${escapeHtml(finding.title ?? "Diagnostic finding")}</strong>
      <p>${escapeHtml(finding.explanation ?? "")}</p>
      <p><strong>Evidence:</strong> ${escapeHtml(finding.evidence ?? "none")}</p>
      ${action}
    </article>
  `;
}

function formatOptionalMetric(value, unit) {
  return value === null || value === undefined ? "unknown" : `${value}${unit}`;
}

function groupCounts(values) {
  const grouped = new Map();
  for (const value of values) {
    grouped.set(value, (grouped.get(value) ?? 0) + 1);
  }

  return [...grouped.entries()]
    .map(([value, count]) => ({ value, count }))
    .sort((a, b) => b.count - a.count || a.value.localeCompare(b.value));
}

function setAnalysisMode(mode) {
  state.analysisMode = mode;
  renderAnalysis(state.analysis);
}

function markdownToHtml(markdown) {
  const lines = String(markdown).split("\n");
  const html = [];
  let inList = false;

  for (const line of lines) {
    const trimmed = line.trim();

    if (!trimmed) {
      if (inList) {
        html.push("</ul>");
        inList = false;
      }
      continue;
    }

    const heading = trimmed.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      if (inList) {
        html.push("</ul>");
        inList = false;
      }
      const level = Math.min(heading[1].length + 2, 4);
      html.push(`<h${level}>${inlineMarkdown(heading[2])}</h${level}>`);
      continue;
    }

    const listItem = trimmed.match(/^[-*]\s+(.+)$/);
    if (listItem) {
      if (!inList) {
        html.push("<ul>");
        inList = true;
      }
      html.push(`<li>${inlineMarkdown(listItem[1])}</li>`);
      continue;
    }

    if (inList) {
      html.push("</ul>");
      inList = false;
    }
    html.push(`<p>${inlineMarkdown(trimmed)}</p>`);
  }

  if (inList) {
    html.push("</ul>");
  }

  return html.join("");
}

function inlineMarkdown(value) {
  return escapeHtml(value)
    .replaceAll(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replaceAll(/`([^`]+)`/g, "<code>$1</code>");
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

el.refresh.addEventListener("click", () => refreshPayload(false));
el.fullBench.addEventListener("click", () => refreshPayload(true));
el.analyze.addEventListener("click", analyzePayload);
el.configForm.addEventListener("submit", saveConfig);
el.analysisRenderedToggle.addEventListener("click", () => setAnalysisMode("rendered"));
el.analysisRawToggle.addEventListener("click", () => setAnalysisMode("raw"));

refreshPayload();
