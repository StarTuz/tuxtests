const invoke = window.__TAURI__.core.invoke;

const state = {
  config: null,
  payload: null,
  selectedDrive: 0,
};

const el = {
  status: document.querySelector("#status"),
  system: document.querySelector("#system"),
  drives: document.querySelector("#drives"),
  driveCount: document.querySelector("#drive-count"),
  details: document.querySelector("#drive-details"),
  diagnostics: document.querySelector("#diagnostics"),
  analysis: document.querySelector("#analysis"),
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
  const pciePath = drive.pcie_path?.length
    ? drive.pcie_path
        .map((path) => {
          const speed = path.current_link_speed ?? "?";
          const width = path.current_link_width ? `x${path.current_link_width}` : "x?";
          const aspm = path.aspm ?? path.aspm_probe_error ?? "ASPM unknown";
          return `- ${path.bdf} ${speed} ${width}: ${aspm}`;
        })
        .join("\n")
    : "No PCIe path in payload.";

  el.details.innerHTML = `
    <dl>
      <dt>Name</dt><dd>${escapeHtml(drive.name)}</dd>
      <dt>Type</dt><dd>${escapeHtml(drive.type)}</dd>
      <dt>Connection</dt><dd>${escapeHtml(drive.connection)}</dd>
      <dt>Mounts</dt><dd>${escapeHtml(mounts)}</dd>
      <dt>Health</dt><dd>${drive.health_ok ? "OK" : "Needs attention"}</dd>
      <dt>Serial</dt><dd>${escapeHtml(drive.serial ?? "unknown")}</dd>
    </dl>
    <h3>PCIe Path</h3>
    <pre>${escapeHtml(pciePath)}</pre>
  `;
}

function renderDiagnostics() {
  const anomalies = state.payload?.kernel_anomalies ?? [];
  el.diagnostics.textContent = anomalies.length ? anomalies.join("\n\n") : "No kernel anomalies in payload.";
}

function renderAnalysis(markdown) {
  el.analysis.textContent = markdown ?? "Run analysis after a payload refresh.";
}

async function refreshPayload(fullBench = false) {
  setBusy(true);
  setStatus(fullBench ? "Collecting full-bench backend payload..." : "Collecting backend payload...");
  try {
    state.config = await invoke("get_config");
    renderConfigForm();
    state.payload = await invoke("get_payload", { fullBench });
    state.selectedDrive = Math.min(state.selectedDrive, Math.max(0, state.payload.drives.length - 1));
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

refreshPayload();
