import { invoke, isTauri } from "@tauri-apps/api/core";
import "./style.css";

type Layout = "english" | "russian";
interface Settings {
  enabled: boolean;
  launchAtLogin: boolean;
  undoShortcut: string;
  excludedApps: string[];
  allowedApps: string[];
  previewConsent: boolean;
  onboardingComplete: boolean;
}
interface Status {
  settings: Settings;
  platform: string;
  permissionGranted: boolean;
  hookActive: boolean;
  englishInstalled: boolean;
  russianInstalled: boolean;
  publicReleaseApproved: boolean;
  developmentBuild: boolean;
  state: string;
  lastError: string | null;
  defaultExclusions: string[];
}
interface Candidate { original: string; replacement: string; target: Layout }

const native = isTauri();
let status: Status = {
  settings: { enabled: false, launchAtLogin: false, undoShortcut: "Meta+Alt+Backspace", excludedApps: [], allowedApps: [], previewConsent: false, onboardingComplete: false },
  platform: "macos", permissionGranted: false, hookActive: false, englishInstalled: false, russianInstalled: false,
  publicReleaseApproved: false, developmentBuild: false, state: "paused", lastError: null, defaultExclusions: [],
};
let saving = false;
let previewRequest = 0;
let ruleSignature = "";
let previewTimer: ReturnType<typeof setTimeout>;

const icons: Record<string, string> = {
  overview: '<rect x="3" y="3" width="7" height="7" rx="2"/><rect x="14" y="3" width="7" height="7" rx="2"/><rect x="3" y="14" width="7" height="7" rx="2"/><rect x="14" y="14" width="7" height="7" rx="2"/>',
  settings: '<path d="M4 7h16M4 17h16"/><circle cx="9" cy="7" r="3"/><circle cx="15" cy="17" r="3"/>',
  apps: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M3 9h18M8 9v11"/>',
  shield: '<path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6l8-3Z"/><path d="m8 12 3 3 5-6"/>',
  arrow: '<path d="M4 8h16l-4-4M20 16H4l4 4"/>',
  chevron: '<path d="m9 5 7 7-7 7"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7v.1"/>',
  keyboard: '<rect x="2" y="5" width="20" height="14" rx="3"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M7 15h10"/>',
};
function icon(name: string) { return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${icons[name] ?? icons.info}</svg>`; }
function element<T extends HTMLElement = HTMLElement>(id: string): T {
  const result = document.getElementById(id);
  if (!result) throw new Error(`Missing UI element: ${id}`);
  return result as T;
}

element("app").innerHTML = `
  <aside class="sidebar">
    <a class="brand" href="#overview" aria-label="CodeSwitch overview"><span class="brand-mark">${icon("arrow")}</span>CodeSwitch</a>
    <div class="nav-caption">WORKSPACE</div>
    <nav aria-label="Main navigation">
      <button data-page="overview" class="nav-item selected">${icon("overview")}Overview</button>
      <button data-page="preferences" class="nav-item">${icon("settings")}Preferences</button>
      <button data-page="exclusions" class="nav-item">${icon("apps")}App exclusions</button>
      <button data-page="about" class="nav-item">${icon("shield")}Privacy & about</button>
    </nav>
    <div class="sidebar-bottom"><div class="local-note">${icon("shield")}On your device. Always.</div><span class="version">CodeSwitch 0.1.0 <span>Preview</span></span></div>
  </aside>
  <div class="workspace">
    <header class="topbar"><span id="page-title">Overview</span><div class="language-pair"><span class="language-dot"></span>EN<span class="pair-arrow">↔</span>RU</div></header>
    <main>
      <div id="browser-notice" class="notice" hidden>You're viewing the interface in a browser. Open the desktop app to check permissions and try correction.</div>
      <div id="error" class="error" role="alert" hidden></div>
      <section id="page-overview" class="page">
        <div class="eyebrow">A LITTLE LESS FRICTION</div>
        <h1>Your words.<br>The right language.</h1>
        <p class="intro">Stay in your flow. CodeSwitch catches the wrong keyboard<br class="wide-break"> layout and puts your words back where they belong.</p>
        <div class="control-card">
          <div class="control-symbol">${icon("arrow")}</div>
          <div class="control-copy"><h2>Automatic correction</h2><p id="correction-description">Paused. Your keyboard is all yours.</p></div>
          <button id="enabled" class="switch" role="switch" aria-checked="false" aria-label="Automatic correction"><span></span></button>
          <div class="control-footer"><span id="status" class="status-pill"><i></i>Paused</span><span>English <span class="pair-arrow">↔</span> Russian</span></div>
        </div>
        <div id="setup" class="setup-card">
          <div class="section-heading"><h2>A quick setup</h2><span id="setup-progress" class="muted">0 of 2 ready</span></div>
          <div class="setup-row"><span id="permission-check" class="step-number">1</span><div><h3>Allow keyboard access</h3><p id="permission-description">Permission lets CodeSwitch correct text in other apps.</p></div><button id="permission" class="button small">Grant access ${icon("chevron")}</button></div>
          <div class="setup-row"><span id="layout-check" class="step-number">2</span><div><h3>Add both keyboard layouts</h3><p id="layout-description">US English and standard Russian ЙЦУКЕН.</p></div><button id="layout-help" class="text-button">How to add ${icon("chevron")}</button></div>
          <div id="layout-instructions" class="help" hidden></div>
          <label id="preview-consent-row" class="preview-consent" hidden><input id="preview-consent" type="checkbox"> <span>I want to test automatic correction in this development build. I understand it may make mistakes.</span></label>
          <div class="setup-bottom"><span id="release-note">Automatic correction is awaiting public release validation.</span><button id="finish-setup" class="button primary small">Finish setup</button></div>
        </div>
        <div class="demo-card">
          <div class="section-heading"><h2>See the switch</h2><span class="tiny-badge">OFFLINE</span></div>
          <p class="muted">Same keys. What you meant to say.</p>
          <div class="demo-conversion"><span class="wrong-word">ghbdtn</span><span class="demo-arrow">${icon("arrow")}</span><span class="right-word" lang="ru">привет<span class="cursor"></span></span></div>
          <div class="try-area"><label for="test-word">Try a word</label><div class="test-controls"><input id="test-word" value="ghbdtn" placeholder="A word in the wrong layout" maxlength="64" autocomplete="off" spellcheck="false" autocapitalize="off"><select id="test-layout" aria-label="Current keyboard layout"><option value="english">Typed in EN</option><option value="russian">Typed in RU</option></select></div><div id="test-result" class="test-result" aria-live="polite">Open the desktop app to try the detector.</div></div>
          <div class="demo-footer">${icon("keyboard")}Correction happens after a word.<span id="undo-hint"><kbd>⌥</kbd><kbd>⌘</kbd><kbd>⌫</kbd> to undo</span></div>
        </div>
        <div class="privacy-line">${icon("shield")}Your typing stays on your device. No cloud. No typing history.</div>
      </section>
      <section id="page-preferences" class="page" hidden>
        <div class="eyebrow">MAKE IT YOURS</div><h1>Quietly, in the<br>background.</h1><p class="intro">A few small preferences. Then, back to your words.</p>
        <div class="panel"><div class="preference-row"><div><h2>Launch at login</h2><p>Keep CodeSwitch in your menu bar or system tray.</p></div><input id="launch-at-login" class="checkbox" type="checkbox" aria-label="Launch at login"></div><div class="preference-row"><div><h2>Undo shortcut</h2><p>Restore the last correction and its previous keyboard layout.<br>Available until you type again or move the caret.</p></div><input id="undo-shortcut" class="shortcut-input" aria-label="Undo shortcut" placeholder="Meta+Alt+Backspace"></div><div class="panel-footer"><span class="muted">Modifiers: Ctrl, Alt, Shift, Meta. Keys: Backspace, F8–F10.</span><button id="save-preferences" class="button primary">Save preferences</button></div></div>
        <div class="info-card">${icon("info")}<p>Correction favors certainty. Short words, names, unfamiliar words, and matches that could belong to either language are left unchanged. Enter and Tab never trigger correction.</p></div>
      </section>
      <section id="page-exclusions" class="page" hidden>
        <div class="eyebrow">A PLACE FOR EVERYTHING</div><h1>Choose where<br>it works.</h1><p class="intro">Terminals and code editors are excluded by default.<br>Allow an app or add an exclusion using its exact identifier.</p>
        <div class="panel app-form"><label for="app-id">App identifier</label><div class="app-controls"><input id="app-id" placeholder="com.example.app or example.exe" maxlength="500" spellcheck="false"><select id="app-rule" aria-label="Rule"><option value="exclude">Exclude app</option><option value="allow">Allow app</option></select><button id="add-app" class="button primary">Add</button></div><p class="muted">macOS: bundle identifier. Windows: executable filename. Explicit exclusions take priority.</p></div>
        <div class="panel"><div class="section-heading"><h2>Your app rules</h2><span id="rule-count" class="muted">0 rules</span></div><div id="custom-apps" class="app-list"></div></div>
        <details class="panel default-apps"><summary>Default exclusions <span class="muted">Terminals & development tools</span></summary><div id="default-apps" class="app-list"></div></details>
      </section>
      <section id="page-about" class="page" hidden>
        <div class="eyebrow">BUILT ON TRUST</div><h1>Your words<br>stay yours.</h1><p class="intro">A small, open-source utility. No account needed.</p>
        <div class="panel privacy-details"><h2>Local by design</h2><p>Detection runs entirely on your device. CodeSwitch has no analytics, accounts, cloud inference, or automatic updater. It never writes your typing to logs or sends it over the network.</p><h2>Only what it needs</h2><p>A short, temporary word buffer and the immediately reversible correction live in memory. CodeSwitch verifies the focused field before making a change. Protected and unsupported fields are skipped.</p><h2>Honest about boundaries</h2><p>Only standard US English and Russian layouts are supported. Custom editors, input methods, remote desktops, and elevated Windows apps may not support correction. Automatic correction remains gated until the release benchmark and platform validation pass.</p><h2>Free & open source</h2><p>CodeSwitch application code is MIT licensed. Word frequencies are from <strong>wordfreq by Robyn Speer</strong>, with data under CC BY-SA 4.0. SUBTLEX authors: <strong>Marc Brysbaert et al.</strong>; SUBTLEX is freely available data. Complete licenses and source attributions ship with the app under <code>licenses/</code>.</p><div class="about-version">CodeSwitch 0.1.0 <span>English ↔ Russian</span></div></div>
      </section>
      <div id="toast" class="toast" role="status" hidden></div>
    </main>
  </div>`;

function showPage(page: string) {
  document.querySelectorAll<HTMLElement>(".page").forEach((section) => { section.hidden = section.id !== `page-${page}`; });
  document.querySelectorAll<HTMLButtonElement>("[data-page]").forEach((button) => {
    const selected = button.dataset.page === page;
    button.classList.toggle("selected", selected);
    button.setAttribute("aria-current", selected ? "page" : "false");
  });
  element("page-title").textContent = { overview: "Overview", preferences: "Preferences", exclusions: "App exclusions", about: "Privacy & about" }[page] ?? "Overview";
  document.querySelector("main")?.scrollTo(0, 0);
}
document.querySelectorAll<HTMLButtonElement>("[data-page]").forEach((button) => button.addEventListener("click", () => showPage(button.dataset.page ?? "overview")));
document.querySelector(".brand")?.addEventListener("click", () => showPage("overview"));

function error(message: string | null) {
  element("error").textContent = message;
  element("error").hidden = !message;
}
function toast(message: string) {
  element("toast").textContent = message;
  element("toast").hidden = false;
  setTimeout(() => { element("toast").hidden = true; }, 2500);
}

function render() {
  const { settings } = status;
  const layouts = status.englishInstalled && status.russianInstalled;
  const available = status.publicReleaseApproved || (status.developmentBuild && settings.previewConsent);
  const canEnable = native && layouts && status.permissionGranted && status.hookActive && settings.onboardingComplete && available;
  const toggle = element<HTMLButtonElement>("enabled");
  toggle.setAttribute("aria-checked", String(settings.enabled));
  toggle.disabled = saving || (!settings.enabled && !canEnable);
  const labels: Record<string, string> = { paused: "Paused", active: "Ready to switch", "needs-permission": "Needs keyboard access", "unsupported-field": "Waiting for a supported field", "excluded-app": "Paused in excluded app" };
  const current = settings.enabled ? (labels[status.state] ?? "Ready to switch") : "Paused";
  element("status").innerHTML = "<i></i>";
  element("status").append(document.createTextNode(current));
  element("status").classList.toggle("active", settings.enabled);
  element("correction-description").textContent = settings.enabled ? "Watching for a clear match after each word." : canEnable ? "Ready when you are. Turn it on to start." : "Paused. Your keyboard is all yours.";
  element("browser-notice").hidden = native;
  element("setup").hidden = settings.onboardingComplete && status.permissionGranted && layouts && available;
  const ready = Number(status.permissionGranted) + Number(layouts);
  element("setup-progress").textContent = `${ready} of 2 ready`;
  for (const [id, complete, number] of [["permission-check", status.permissionGranted, "1"], ["layout-check", layouts, "2"]] as const) {
    element(id).innerHTML = complete ? icon("check") : number;
    element(id).classList.toggle("complete", complete);
  }
  const permission = element<HTMLButtonElement>("permission");
  permission.disabled = !native || status.permissionGranted;
  permission.textContent = status.permissionGranted ? "Allowed" : status.platform === "windows" ? "Check access" : "Grant access";
  element("permission-description").textContent = status.platform === "windows" ? "Runs as a standard user. Elevated apps are skipped." : "Enable CodeSwitch in System Settings → Privacy & Security → Accessibility.";
  element("layout-description").textContent = `${status.platform === "macos" ? "U.S. / ABC" : "US English"} ${status.englishInstalled ? "✓" : "— missing"} · ${status.platform === "macos" ? "Russian — PC" : "Russian ЙЦУКЕН"} ${status.russianInstalled ? "✓" : "— missing"}`;
  element("layout-instructions").textContent = status.platform === "windows" ? "Open Settings → Time & language → Language & region. Add English (United States) and Russian. In Language options, select the US and standard Russian keyboards. Other layout variants are not supported in v1." : "Open System Settings → Keyboard → Text Input → Edit. Add English → U.S. (ABC also works) and Russian → Russian — PC. The Apple Russian and phonetic variants have different key mappings and are not supported in v1.";
  element("preview-consent-row").hidden = !status.developmentBuild || status.publicReleaseApproved;
  element<HTMLInputElement>("preview-consent").checked = settings.previewConsent;
  element("release-note").textContent = status.publicReleaseApproved ? "You're ready. Enable correction whenever you like." : status.developmentBuild ? "Development preview · public validation is still in progress." : "Automatic correction is awaiting public release validation. The test box remains available.";
  element<HTMLButtonElement>("finish-setup").disabled = !native || !layouts || !status.permissionGranted || saving;
  element<HTMLButtonElement>("finish-setup").textContent = settings.onboardingComplete ? "Setup complete" : "Finish setup";
  const launch = element<HTMLInputElement>("launch-at-login");
  if (document.activeElement !== launch && !launch.dataset.dirty) launch.checked = settings.launchAtLogin;
  const shortcut = element<HTMLInputElement>("undo-shortcut");
  if (document.activeElement !== shortcut && !shortcut.dataset.dirty) shortcut.value = settings.undoShortcut;
  element<HTMLButtonElement>("save-preferences").disabled = !native || saving;
  element<HTMLButtonElement>("add-app").disabled = !native || saving;
  const hint = element("undo-hint");
  hint.replaceChildren();
  for (const part of settings.undoShortcut.split("+")) {
    const key = document.createElement("kbd");
    key.textContent = status.platform === "macos" ? ({ Meta: "⌘", Alt: "⌥", Ctrl: "⌃", Shift: "⇧", Backspace: "⌫" }[part] ?? part) : part;
    hint.append(key);
  }
  hint.append(document.createTextNode(" to undo"));
  renderApps();
  if (status.lastError) error(status.lastError);
}

function renderApps() {
  const signature = JSON.stringify([status.settings.excludedApps, status.settings.allowedApps, status.defaultExclusions, saving]);
  if (signature === ruleSignature) return;
  ruleSignature = signature;
  const custom = element("custom-apps");
  custom.replaceChildren();
  const rules = [...status.settings.excludedApps.map((app) => ({ app, rule: "Excluded" })), ...status.settings.allowedApps.map((app) => ({ app, rule: "Allowed" }))];
  element("rule-count").textContent = `${rules.length} ${rules.length === 1 ? "rule" : "rules"}`;
  if (!rules.length) { const note = document.createElement("p"); note.className = "muted empty-state"; note.textContent = "No custom rules yet. Default exclusions are already in place."; custom.append(note); }
  for (const entry of rules) {
    const row = document.createElement("div"); row.className = "app-row";
    const name = document.createElement("code"); name.textContent = entry.app;
    const badge = document.createElement("span"); badge.className = "app-badge"; badge.textContent = entry.rule;
    const remove = document.createElement("button"); remove.className = "text-button"; remove.textContent = "Remove"; remove.disabled = saving;
    remove.addEventListener("click", () => { void update({ excludedApps: status.settings.excludedApps.filter((app) => app !== entry.app), allowedApps: status.settings.allowedApps.filter((app) => app !== entry.app) }); });
    row.append(name, badge, remove); custom.append(row);
  }
  const defaults = element("default-apps"); defaults.replaceChildren();
  for (const app of status.defaultExclusions) {
    const row = document.createElement("div"); row.className = "app-row";
    const name = document.createElement("code"); name.textContent = app;
    const allow = document.createElement("button"); allow.className = "text-button";
    const allowed = status.settings.allowedApps.some((item) => item.toLowerCase() === app.toLowerCase());
    allow.textContent = allowed ? "Allowed" : "Allow app"; allow.disabled = allowed || saving;
    allow.addEventListener("click", () => { void update({ allowedApps: [...status.settings.allowedApps, app] }); });
    row.append(name, allow); defaults.append(row);
  }
}

async function refresh() {
  if (!native || saving) return;
  try { status = await invoke<Status>("get_status"); render(); }
  catch (cause) { error(String(cause)); }
}

async function update(changes: Partial<Settings>): Promise<boolean> {
  if (!native || saving) return false;
  saving = true; render(); error(null);
  try {
    await invoke("save_settings", { settings: { ...status.settings, ...changes } });
    status.settings = { ...status.settings, ...changes };
    return true;
  } catch (cause) { error(String(cause)); return false; }
  finally { saving = false; await refresh(); }
}

element("enabled").addEventListener("click", () => { void update({ enabled: !status.settings.enabled }); });
element("permission").addEventListener("click", () => {
  void invoke("request_permission").then(() => refresh()).catch((cause) => error(String(cause)));
});
element("layout-help").addEventListener("click", () => { element("layout-instructions").hidden = !element("layout-instructions").hidden; });
element("finish-setup").addEventListener("click", () => { void update({ onboardingComplete: true }).then((saved) => { if (saved) toast("Setup complete. You can now choose to enable correction."); }); });
element("preview-consent").addEventListener("change", () => {
  const checked = element<HTMLInputElement>("preview-consent").checked;
  void update({ previewConsent: checked, ...(!checked ? { enabled: false } : {}) });
});
element("undo-shortcut").addEventListener("input", () => { element("undo-shortcut").dataset.dirty = "true"; });
element("launch-at-login").addEventListener("change", () => { element("launch-at-login").dataset.dirty = "true"; });
element("save-preferences").addEventListener("click", () => {
  void update({ launchAtLogin: element<HTMLInputElement>("launch-at-login").checked, undoShortcut: element<HTMLInputElement>("undo-shortcut").value.trim() }).then((saved) => { if (saved) { delete element("undo-shortcut").dataset.dirty; delete element("launch-at-login").dataset.dirty; toast("Preferences saved"); } });
});
element("add-app").addEventListener("click", () => {
  const app = element<HTMLInputElement>("app-id").value.trim();
  if (!app) { error("Enter an exact app identifier first."); return; }
  const rule = element<HTMLSelectElement>("app-rule").value;
  const excludedApps = status.settings.excludedApps.filter((item) => item.toLowerCase() !== app.toLowerCase());
  const allowedApps = status.settings.allowedApps.filter((item) => item.toLowerCase() !== app.toLowerCase());
  (rule === "exclude" ? excludedApps : allowedApps).push(app);
  void update({ excludedApps, allowedApps }).then((saved) => { if (saved) element<HTMLInputElement>("app-id").value = ""; });
});

async function preview() {
  if (!native) return;
  const request = ++previewRequest;
  const word = element<HTMLInputElement>("test-word").value;
  const layout = element<HTMLSelectElement>("test-layout").value as Layout;
  try {
    const result = await invoke<Candidate | null>("preview_word", { word, layout });
    if (request !== previewRequest) return;
    const output = element("test-result"); output.replaceChildren();
    if (result) { const corrected = document.createElement("strong"); corrected.textContent = result.replacement; output.append("Would switch to ", corrected, ` · ${result.target === "russian" ? "Russian" : "English"}`); }
    else output.textContent = word ? "Left unchanged — no clear match in the other layout." : "Type a word to try the detector.";
  } catch (cause) { if (request === previewRequest) error(String(cause)); }
}
for (const id of ["test-word", "test-layout"]) element(id).addEventListener("input", () => { clearTimeout(previewTimer); previewTimer = setTimeout(() => { void preview(); }, 180); });

render();
void refresh().then(() => preview());
setInterval(() => { if (!document.hidden) void refresh(); }, 1500);
