import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { language, plural, resolveLanguage, setLanguage, t, translateDocument, type Key, type LanguageSetting } from "./i18n";
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
  automaticUpdateChecks: boolean;
  convertShortcut: string;
  ignoredWords: string[];
  learnFromUndo: boolean;
  switchLayout: boolean;
  language: LanguageSetting;
  theme: ThemeSetting;
}
type ThemeSetting = "system" | "light" | "dark";
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
  version: string;
  updates: UpdateStatus;
  suspendedApps: string[];
  corrections: number;
}
interface UpdateStatus { phase: string; version: string | null; notes: string | null; downloaded: number; total: number | null; message: string | null }
interface Candidate { original: string; replacement: string; target: Layout }

const native = isTauri();
let status: Status = {
  settings: { enabled: false, launchAtLogin: false, undoShortcut: "Meta+Alt+Backspace", excludedApps: [], allowedApps: [], previewConsent: false, onboardingComplete: false, automaticUpdateChecks: true, convertShortcut: "Ctrl+Meta+Backspace", ignoredWords: [], learnFromUndo: true, switchLayout: true, language: "system", theme: "system" },
  platform: "macos", permissionGranted: false, hookActive: false, englishInstalled: false, russianInstalled: false,
  publicReleaseApproved: false, developmentBuild: false, state: "paused", lastError: null, defaultExclusions: [],
  version: "0.1.0", updates: { phase: "", version: null, notes: null, downloaded: 0, total: null, message: null },
  suspendedApps: [], corrections: 0,
};
let saving = false;
let previewRequest = 0;
let ruleSignature = "";
let wordSignature = "";
let suspendedSignature = "";
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
  words: '<path d="M4 6h16M4 12h10M4 18h7"/><path d="m15 15 5 5M20 15l-5 5"/>',
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
    <a class="brand" href="#overview" data-i18n-label="brand.label"><span class="brand-mark">${icon("arrow")}</span>CodeSwitch</a>
    <div class="nav-caption" data-i18n="nav.caption"></div>
    <nav aria-label="Main navigation">
      <button data-page="overview" class="nav-item selected">${icon("overview")}<span data-i18n="nav.overview"></span></button>
      <button data-page="preferences" class="nav-item">${icon("settings")}<span data-i18n="nav.preferences"></span></button>
      <button data-page="exclusions" class="nav-item">${icon("apps")}<span data-i18n="nav.exclusions"></span></button>
      <button data-page="words" class="nav-item">${icon("words")}<span data-i18n="nav.words"></span></button>
      <button data-page="about" class="nav-item">${icon("shield")}<span data-i18n="nav.about"></span></button>
    </nav>
    <div class="sidebar-bottom"><div class="local-note">${icon("shield")}<span data-i18n="sidebar.local"></span></div><span class="version"><span id="app-version">CodeSwitch 0.1.0</span> <span data-i18n="sidebar.preview"></span></span></div>
  </aside>
  <div class="workspace">
    <header class="topbar"><span id="page-title"></span><div class="language-pair"><span class="language-dot"></span>EN<span class="pair-arrow">↔</span>RU</div></header>
    <main>
      <div id="browser-notice" class="notice" data-i18n="browser.notice" hidden></div>
      <div id="error" class="error" role="alert" hidden></div>
      <div id="suspended" role="status"></div>
      <div id="update-notice" class="notice update-notice" role="status" hidden><span id="update-notice-text"></span><button id="view-update" class="text-button" data-i18n="update.view"></button></div>
      <section id="page-overview" class="page">
        <div class="eyebrow" data-i18n="overview.eyebrow"></div>
        <h1 data-i18n="overview.title"></h1>
        <p class="intro" data-i18n="overview.intro"></p>
        <div class="control-card">
          <div class="control-symbol">${icon("arrow")}</div>
          <div class="control-copy"><h2 data-i18n="control.title"></h2><p id="correction-description"></p></div>
          <button id="enabled" class="switch" role="switch" aria-checked="false" data-i18n-label="control.title"><span></span></button>
          <div class="control-footer"><span id="status" class="status-pill"><i></i></span><span id="correction-count"></span></div>
        </div>
        <div id="setup" class="setup-card">
          <div class="section-heading"><h2 data-i18n="setup.title"></h2><span id="setup-progress" class="muted"></span></div>
          <div class="setup-row"><span id="permission-check" class="step-number">1</span><div><h3 data-i18n="setup.permission"></h3><p id="permission-description"></p></div><button id="permission" class="button small"></button></div>
          <div class="setup-row"><span id="layout-check" class="step-number">2</span><div><h3 data-i18n="setup.layouts"></h3><p id="layout-description"></p></div><button id="layout-help" class="text-button"><span data-i18n="setup.howTo"></span> ${icon("chevron")}</button></div>
          <div id="layout-instructions" class="help" hidden></div>
          <label id="preview-consent-row" class="preview-consent" hidden><input id="preview-consent" type="checkbox"> <span data-i18n="setup.consent"></span></label>
          <div class="setup-bottom"><span id="release-note"></span><button id="finish-setup" class="button primary small"></button></div>
        </div>
        <div class="demo-card">
          <div class="section-heading"><h2 data-i18n="demo.title"></h2><span class="tiny-badge" data-i18n="demo.offline"></span></div>
          <p class="muted" data-i18n="demo.subtitle"></p>
          <div class="demo-conversion"><span class="wrong-word">ghbdtn</span><span class="demo-arrow">${icon("arrow")}</span><span class="right-word" lang="ru">привет<span class="cursor"></span></span></div>
          <div class="try-area"><label for="test-word" data-i18n="demo.try"></label><div class="test-controls"><input id="test-word" value="ghbdtn" data-i18n-placeholder="demo.placeholder" maxlength="64" autocomplete="off" spellcheck="false" autocapitalize="off"><select id="test-layout" data-i18n-label="demo.layout"><option value="english" data-i18n="demo.typedEn"></option><option value="russian" data-i18n="demo.typedRu"></option></select></div><div id="test-result" class="test-result" aria-live="polite"></div></div>
          <div class="demo-footer">${icon("keyboard")}<span class="demo-after" data-i18n="demo.after"></span><span id="convert-hint"></span><span id="undo-hint"></span></div>
        </div>
        <div class="privacy-line">${icon("shield")}<span data-i18n="privacy.line"></span></div>
      </section>
      <section id="page-preferences" class="page" hidden>
        <div class="eyebrow" data-i18n="prefs.eyebrow"></div><h1 data-i18n="prefs.title"></h1><p class="intro" data-i18n="prefs.intro"></p>
        <div class="panel interface-panel">
          <div class="section-heading"><h2 data-i18n="interface.title"></h2></div>
          <div class="preference-row"><div><h3 data-i18n="interface.language"></h3><p data-i18n="interface.languageHint"></p></div><div class="segmented" id="language-choice" role="radiogroup" data-i18n-label="interface.language"><button role="radio" data-value="system" data-i18n="interface.system"></button><button role="radio" data-value="en" lang="en">English</button><button role="radio" data-value="ru" lang="ru">Русский</button></div></div>
          <div class="preference-row"><div><h3 data-i18n="interface.appearance"></h3><p data-i18n="interface.appearanceHint"></p></div><div class="segmented" id="theme-choice" role="radiogroup" data-i18n-label="interface.appearance"><button role="radio" data-value="system" data-i18n="interface.system"></button><button role="radio" data-value="light" data-i18n="interface.light"></button><button role="radio" data-value="dark" data-i18n="interface.dark"></button></div></div>
        </div>
        <div class="panel"><div class="preference-row"><div><h2 data-i18n="prefs.switchLayout"></h2><p data-i18n="prefs.switchLayoutHint"></p></div><input id="switch-layout" class="checkbox" type="checkbox" data-i18n-label="prefs.switchLayout"></div></div>
        <div class="panel"><div class="preference-row"><div><h2 data-i18n="prefs.launch"></h2><p data-i18n="prefs.launchHint"></p></div><input id="launch-at-login" class="checkbox" type="checkbox" data-i18n-label="prefs.launch"></div><div class="preference-row"><div><h2 data-i18n="prefs.undo"></h2><p data-i18n="prefs.undoHint"></p></div><input id="undo-shortcut" class="shortcut-input" data-i18n-label="prefs.undo" placeholder="Meta+Alt+Backspace"></div><div class="preference-row"><div><h2 data-i18n="prefs.convert"></h2><p data-i18n="prefs.convertHint"></p></div><input id="convert-shortcut" class="shortcut-input" data-i18n-label="prefs.convert" data-i18n-placeholder="prefs.off"></div><div class="panel-footer"><span class="muted" data-i18n="prefs.keys"></span><button id="save-preferences" class="button primary" data-i18n="prefs.save"></button></div></div>
        <div class="info-card">${icon("info")}<p data-i18n="prefs.info"></p></div>
        <div class="panel updates-panel">
          <div class="section-heading"><h2 data-i18n="updates.title"></h2><span id="update-current-version" class="muted"></span></div>
          <div class="preference-row"><div><h3 data-i18n="updates.auto"></h3><p data-i18n="updates.autoHint"></p></div><input id="automatic-update-checks" class="checkbox" type="checkbox" data-i18n-label="updates.autoLabel"></div>
          <p id="update-status" class="update-status" role="status" aria-live="polite"></p>
          <progress id="update-progress" max="100" hidden data-i18n-label="updates.progress"></progress>
          <p id="update-notes" class="update-notes" hidden></p>
          <div class="update-actions"><button id="check-updates" class="button" data-i18n="updates.check"></button><button id="install-update" class="button primary" data-i18n="updates.install" hidden></button></div>
          <p class="muted update-footnote" data-i18n="updates.footnote"></p>
        </div>
      </section>
      <section id="page-exclusions" class="page" hidden>
        <div class="eyebrow" data-i18n="apps.eyebrow"></div><h1 data-i18n="apps.title"></h1><p class="intro" data-i18n="apps.intro"></p>
        <div class="panel app-form"><label for="app-id" data-i18n="apps.identifier"></label><div class="app-controls"><input id="app-id" data-i18n-placeholder="apps.placeholder" maxlength="500" spellcheck="false"><select id="app-rule" data-i18n-label="apps.rule"><option value="exclude" data-i18n="apps.exclude"></option><option value="allow" data-i18n="apps.allow"></option></select><button id="add-app" class="button primary" data-i18n="apps.add"></button></div><p class="muted" data-i18n="apps.help"></p></div>
        <div class="panel"><div class="section-heading"><h2 data-i18n="apps.yours"></h2><span id="rule-count" class="muted"></span></div><div id="custom-apps" class="app-list"></div></div>
        <details class="panel default-apps"><summary><span data-i18n="apps.defaults"></span> <span class="muted" data-i18n="apps.defaultsHint"></span></summary><div id="default-apps" class="app-list"></div></details>
      </section>
      <section id="page-words" class="page" hidden>
        <div class="eyebrow" data-i18n="words.eyebrow"></div><h1 data-i18n="words.title"></h1><p class="intro" data-i18n="words.intro"></p>
        <div class="panel"><div class="preference-row"><div><h2 data-i18n="words.learn"></h2><p data-i18n="words.learnHint"></p></div><input id="learn-from-undo" class="checkbox" type="checkbox" data-i18n-label="words.learn"></div></div>
        <div class="panel app-form"><label for="word-entry" data-i18n="words.word"></label><div class="app-controls"><input id="word-entry" data-i18n-placeholder="words.placeholder" maxlength="64" autocomplete="off" spellcheck="false" autocapitalize="off"><button id="add-word" class="button primary" data-i18n="apps.add"></button></div><p class="muted" data-i18n="words.help"></p></div>
        <div class="panel"><div class="section-heading"><h2 data-i18n="words.yours"></h2><span><span id="word-count" class="muted"></span> <button id="clear-words" class="text-button" data-i18n="words.clear" hidden></button></span></div><div id="word-list" class="app-list"></div></div>
      </section>
      <section id="page-about" class="page" hidden>
        <div class="eyebrow" data-i18n="about.eyebrow"></div><h1 data-i18n="about.title"></h1><p class="intro" data-i18n="about.intro"></p>
        <div class="panel privacy-details"><h2 data-i18n="about.localTitle"></h2><p data-i18n="about.local"></p><h2 data-i18n="about.updatesTitle"></h2><p data-i18n="about.updates"></p><h2 data-i18n="about.needsTitle"></h2><p data-i18n="about.needs"></p><h2 data-i18n="about.boundsTitle"></h2><p data-i18n="about.bounds"></p><h2 data-i18n="about.openTitle"></h2><p data-i18n="about.open"></p><div class="about-version"><span id="about-version">CodeSwitch 0.1.0</span><span data-i18n="about.pair"></span></div></div>
      </section>
      <div id="toast" class="toast" role="status" hidden></div>
    </main>
  </div>`;

let currentPage = "overview";
function showPage(page: string) {
  currentPage = page;
  document.querySelectorAll<HTMLElement>(".page").forEach((section) => { section.hidden = section.id !== `page-${page}`; });
  document.querySelectorAll<HTMLButtonElement>("[data-page]").forEach((button) => {
    const selected = button.dataset.page === page;
    button.classList.toggle("selected", selected);
    button.setAttribute("aria-current", selected ? "page" : "false");
  });
  const titles: Record<string, Key> = { overview: "nav.overview", preferences: "nav.preferences", exclusions: "nav.exclusions", words: "nav.words", about: "nav.about" };
  element("page-title").textContent = t(titles[page] ?? "nav.overview");
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
  renderInterface();
  const { settings } = status;
  const layouts = status.englishInstalled && status.russianInstalled;
  const available = status.publicReleaseApproved || (status.developmentBuild && settings.previewConsent);
  const canEnable = native && layouts && status.permissionGranted && status.hookActive && settings.onboardingComplete && available;
  const toggle = element<HTMLButtonElement>("enabled");
  toggle.setAttribute("aria-checked", String(settings.enabled));
  toggle.disabled = saving || status.state === "updating" || (!settings.enabled && !canEnable);
  const states = ["paused", "active", "updating", "needs-permission", "unsupported-field", "excluded-app", "suspended-app"];
  const current = settings.enabled ? t(states.includes(status.state) ? `state.${status.state}` as Key : "state.active") : t("state.paused");
  element("status").innerHTML = "<i></i>";
  element("status").append(document.createTextNode(current));
  element("status").classList.toggle("active", settings.enabled && status.state === "active");
  const count = element("correction-count");
  if (status.corrections > 0) count.textContent = t("count.corrected", { n: status.corrections, words: plural(status.corrections, "word") });
  else count.replaceChildren(`${t("pair.english")} `, Object.assign(document.createElement("span"), { className: "pair-arrow", textContent: "↔" }), ` ${t("pair.russian")}`);
  element("correction-description").textContent = t(status.state === "updating" ? "control.updating" : settings.enabled ? "control.watching" : canEnable ? "control.ready" : "control.paused");
  element("browser-notice").hidden = native;
  element("setup").hidden = settings.onboardingComplete && status.permissionGranted && layouts && available;
  const ready = Number(status.permissionGranted) + Number(layouts);
  element("setup-progress").textContent = t("setup.progress", { n: ready });
  for (const [id, complete, number] of [["permission-check", status.permissionGranted, "1"], ["layout-check", layouts, "2"]] as const) {
    element(id).innerHTML = complete ? icon("check") : number;
    element(id).classList.toggle("complete", complete);
  }
  const permission = element<HTMLButtonElement>("permission");
  permission.disabled = !native || status.permissionGranted;
  const windows = status.platform === "windows";
  if (status.permissionGranted) permission.textContent = t("setup.allowed");
  else permission.innerHTML = `${t(windows ? "setup.check" : "setup.grant")} ${icon("chevron")}`;
  element("permission-description").textContent = t(!native ? "setup.permissionIntro" : windows ? "setup.permissionWindows" : "setup.permissionMac");
  element("layout-description").textContent = !native ? t("setup.layoutsIntro") : `${t(windows ? "setup.layoutEnWin" : "setup.layoutEnMac")} ${status.englishInstalled ? "✓" : t("setup.missing")} · ${t(windows ? "setup.layoutRuWin" : "setup.layoutRuMac")} ${status.russianInstalled ? "✓" : t("setup.missing")}`;
  element("layout-instructions").textContent = t(windows ? "setup.helpWindows" : "setup.helpMac");
  element("preview-consent-row").hidden = !status.developmentBuild || status.publicReleaseApproved;
  element<HTMLInputElement>("preview-consent").checked = settings.previewConsent;
  element("release-note").textContent = t(status.publicReleaseApproved ? "setup.releaseReady" : status.developmentBuild ? "setup.releasePreview" : "setup.releasePending");
  element<HTMLButtonElement>("finish-setup").disabled = !native || !layouts || !status.permissionGranted || saving;
  element<HTMLButtonElement>("finish-setup").textContent = t(settings.onboardingComplete ? "setup.complete" : "setup.finish");
  const launch = element<HTMLInputElement>("launch-at-login");
  if (document.activeElement !== launch && !launch.dataset.dirty) launch.checked = settings.launchAtLogin;
  const shortcut = element<HTMLInputElement>("undo-shortcut");
  if (document.activeElement !== shortcut && !shortcut.dataset.dirty) shortcut.value = settings.undoShortcut;
  const convert = element<HTMLInputElement>("convert-shortcut");
  if (document.activeElement !== convert && !convert.dataset.dirty) convert.value = settings.convertShortcut;
  element<HTMLInputElement>("learn-from-undo").checked = settings.learnFromUndo;
  element<HTMLInputElement>("learn-from-undo").disabled = !native || saving;
  element<HTMLInputElement>("switch-layout").checked = settings.switchLayout;
  element<HTMLInputElement>("switch-layout").disabled = !native || saving;
  element<HTMLButtonElement>("add-word").disabled = !native || saving;
  element<HTMLButtonElement>("save-preferences").disabled = !native || saving;
  element<HTMLButtonElement>("add-app").disabled = !native || saving;
  shortcutHint(element("undo-hint"), settings.undoShortcut, t("hint.undo"));
  shortcutHint(element("convert-hint"), settings.convertShortcut, t("hint.convert"));
  renderApps();
  renderWords();
  renderSuspended();
  renderUpdates();
  if (status.lastError) error(status.lastError);
}

function shortcutHint(hint: HTMLElement, shortcut: string, label: string) {
  hint.replaceChildren();
  hint.hidden = !shortcut;
  if (!shortcut) return;
  for (const part of shortcut.split("+")) {
    const key = document.createElement("kbd");
    key.textContent = status.platform === "macos" ? ({ Meta: "⌘", Alt: "⌥", Ctrl: "⌃", Shift: "⇧", Backspace: "⌫" }[part] ?? part) : part;
    hint.append(key);
  }
  hint.append(document.createTextNode(label));
}

function renderSuspended() {
  const signature = JSON.stringify([status.suspendedApps, saving, language()]);
  if (signature === suspendedSignature) return;
  suspendedSignature = signature;
  const container = element("suspended");
  container.replaceChildren();
  for (const app of status.suspendedApps) {
    const notice = document.createElement("div"); notice.className = "notice update-notice";
    const text = document.createElement("span");
    const name = document.createElement("code"); name.textContent = app;
    text.append(t("suspended.before"), name, t("suspended.after"));
    const resume = document.createElement("button"); resume.className = "text-button"; resume.textContent = t("suspended.resume"); resume.disabled = !native || saving;
    resume.addEventListener("click", () => { void invoke("resume_app", { app }).then(() => { error(null); return refresh(); }).catch((cause) => error(String(cause))); });
    notice.append(text, resume); container.append(notice);
  }
}

function renderWords() {
  const signature = JSON.stringify([status.settings.ignoredWords, saving, language()]);
  if (signature === wordSignature) return;
  wordSignature = signature;
  const words = status.settings.ignoredWords;
  element("word-count").textContent = `${words.length} ${plural(words.length, "word")}`;
  element("clear-words").hidden = !words.length;
  element<HTMLButtonElement>("clear-words").disabled = !native || saving;
  const list = element("word-list"); list.replaceChildren();
  if (!words.length) { const note = document.createElement("p"); note.className = "muted empty-state"; note.textContent = t("words.empty"); list.append(note); }
  for (const word of [...words].reverse()) {
    const row = document.createElement("div"); row.className = "app-row";
    const name = document.createElement("code"); name.textContent = word;
    const remove = document.createElement("button"); remove.className = "text-button"; remove.textContent = t("apps.remove"); remove.disabled = saving;
    remove.addEventListener("click", () => { void update({ ignoredWords: status.settings.ignoredWords.filter((item) => item !== word) }); });
    row.append(name, remove); list.append(row);
  }
}

function renderUpdates() {
  const update = status.updates;
  const busy = ["checking", "downloading", "installing"].includes(update.phase);
  for (const id of ["app-version", "about-version"]) element(id).textContent = `CodeSwitch ${status.version}`;
  element("update-current-version").textContent = t("updates.installed", { v: status.version });
  element<HTMLInputElement>("automatic-update-checks").checked = status.settings.automaticUpdateChecks;
  element<HTMLInputElement>("automatic-update-checks").disabled = !native || saving;
  element<HTMLButtonElement>("check-updates").disabled = !native || busy;
  const install = element<HTMLButtonElement>("install-update");
  install.hidden = !update.version;
  install.disabled = !native || busy;
  const labels: Record<string, string> = {
    checking: t("updates.checking"), current: t("updates.current"),
    available: t("updates.available", { v: update.version ?? "" }), downloading: t("updates.downloading"),
    installing: t("updates.installing"),
  };
  element("update-status").textContent = update.message ?? labels[update.phase] ?? t("updates.idle");
  element("update-notes").textContent = update.notes;
  element("update-notes").hidden = !update.notes;
  const progress = element<HTMLProgressElement>("update-progress");
  progress.hidden = update.phase !== "downloading";
  if (update.total) progress.value = Math.min(100, update.downloaded / update.total * 100);
  else progress.removeAttribute("value");
  element("update-notice").hidden = !update.version;
  element("update-notice-text").textContent = t("updates.available", { v: update.version ?? "" });
}

function renderApps() {
  const signature = JSON.stringify([status.settings.excludedApps, status.settings.allowedApps, status.defaultExclusions, saving, language()]);
  if (signature === ruleSignature) return;
  ruleSignature = signature;
  const custom = element("custom-apps");
  custom.replaceChildren();
  const rules = [...status.settings.excludedApps.map((app) => ({ app, rule: t("apps.excluded") })), ...status.settings.allowedApps.map((app) => ({ app, rule: t("apps.allowed") }))];
  element("rule-count").textContent = `${rules.length} ${plural(rules.length, "rule")}`;
  if (!rules.length) { const note = document.createElement("p"); note.className = "muted empty-state"; note.textContent = t("apps.empty"); custom.append(note); }
  for (const entry of rules) {
    const row = document.createElement("div"); row.className = "app-row";
    const name = document.createElement("code"); name.textContent = entry.app;
    const badge = document.createElement("span"); badge.className = "app-badge"; badge.textContent = entry.rule;
    const remove = document.createElement("button"); remove.className = "text-button"; remove.textContent = t("apps.remove"); remove.disabled = saving;
    remove.addEventListener("click", () => { void update({ excludedApps: status.settings.excludedApps.filter((app) => app !== entry.app), allowedApps: status.settings.allowedApps.filter((app) => app !== entry.app) }); });
    row.append(name, badge, remove); custom.append(row);
  }
  const defaults = element("default-apps"); defaults.replaceChildren();
  for (const app of status.defaultExclusions) {
    const row = document.createElement("div"); row.className = "app-row";
    const name = document.createElement("code"); name.textContent = app;
    const allow = document.createElement("button"); allow.className = "text-button";
    const allowed = status.settings.allowedApps.some((item) => item.toLowerCase() === app.toLowerCase());
    allow.textContent = t(allowed ? "apps.allowed" : "apps.allow"); allow.disabled = allowed || saving;
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
element("finish-setup").addEventListener("click", () => { void update({ onboardingComplete: true }).then((saved) => { if (saved) toast(t("setup.toast")); }); });
element("preview-consent").addEventListener("change", () => {
  const checked = element<HTMLInputElement>("preview-consent").checked;
  void update({ previewConsent: checked, ...(!checked ? { enabled: false } : {}) });
});
element("undo-shortcut").addEventListener("input", () => { element("undo-shortcut").dataset.dirty = "true"; });
element("convert-shortcut").addEventListener("input", () => { element("convert-shortcut").dataset.dirty = "true"; });
element("launch-at-login").addEventListener("change", () => { element("launch-at-login").dataset.dirty = "true"; });
element("save-preferences").addEventListener("click", () => {
  void update({ launchAtLogin: element<HTMLInputElement>("launch-at-login").checked, undoShortcut: element<HTMLInputElement>("undo-shortcut").value.trim(), convertShortcut: element<HTMLInputElement>("convert-shortcut").value.trim() }).then((saved) => { if (saved) { for (const id of ["undo-shortcut", "convert-shortcut", "launch-at-login"]) delete element(id).dataset.dirty; toast(t("prefs.saved")); } });
});
element("view-update").addEventListener("click", () => showPage("preferences"));
element("check-updates").addEventListener("click", () => {
  status.updates.phase = "checking"; status.updates.message = null; renderUpdates();
  void invoke<UpdateStatus>("check_for_updates").then((updates) => { status.updates = updates; renderUpdates(); })
    .catch(() => { status.updates.phase = "error"; status.updates.message = t("updates.failed"); renderUpdates(); });
});
element("install-update").addEventListener("click", () => {
  status.updates.phase = "downloading"; status.updates.message = null; renderUpdates();
  void invoke("install_update").catch(() => refresh());
});
element("automatic-update-checks").addEventListener("change", () => {
  const enabled = element<HTMLInputElement>("automatic-update-checks").checked;
  saving = true; render();
  void invoke("set_update_checks", { enabled }).catch((cause) => error(String(cause)))
    .finally(() => { saving = false; void refresh(); });
});
if (native) void listen("show-updates", () => showPage("preferences"));
element("add-app").addEventListener("click", () => {
  const app = element<HTMLInputElement>("app-id").value.trim();
  if (!app) { error(t("apps.enterId")); return; }
  const rule = element<HTMLSelectElement>("app-rule").value;
  const excludedApps = status.settings.excludedApps.filter((item) => item.toLowerCase() !== app.toLowerCase());
  const allowedApps = status.settings.allowedApps.filter((item) => item.toLowerCase() !== app.toLowerCase());
  (rule === "exclude" ? excludedApps : allowedApps).push(app);
  void update({ excludedApps, allowedApps }).then((saved) => { if (saved) element<HTMLInputElement>("app-id").value = ""; });
});

element("learn-from-undo").addEventListener("change", () => { void update({ learnFromUndo: element<HTMLInputElement>("learn-from-undo").checked }); });
element("switch-layout").addEventListener("change", () => { void update({ switchLayout: element<HTMLInputElement>("switch-layout").checked }); });
function addWord() {
  const word = element<HTMLInputElement>("word-entry").value.trim();
  if (!word) { error(t("words.enter")); return; }
  if (/\s/.test(word)) { error(t("words.one")); return; }
  const key = (value: string) => value.toLowerCase().replace(/^[.,!?:;'"()[\]{}«»“”]+|[.,!?:;'"()[\]{}«»“”]+$/g, "");
  if (status.settings.ignoredWords.some((item) => key(item) === key(word))) { element<HTMLInputElement>("word-entry").value = ""; toast(t("words.already")); return; }
  void update({ ignoredWords: [...status.settings.ignoredWords, word] }).then((saved) => { if (saved) element<HTMLInputElement>("word-entry").value = ""; });
}
element("add-word").addEventListener("click", addWord);
element("word-entry").addEventListener("keydown", (event) => { if (event.key === "Enter") addWord(); });
element("clear-words").addEventListener("click", () => {
  if (window.confirm(t("words.confirmClear"))) void update({ ignoredWords: [] });
});

let lastPreview: { word: string; result: Candidate | null } | null = null;
function renderPreview() {
  const output = element("test-result"); output.replaceChildren();
  if (!native || !lastPreview) { output.textContent = t("demo.openApp"); return; }
  const { word, result } = lastPreview;
  if (result) { const corrected = document.createElement("strong"); corrected.textContent = result.replacement; output.append(t("demo.wouldSwitch"), corrected, ` · ${t(result.target === "russian" ? "demo.toRussian" : "demo.toEnglish")}`); }
  else output.textContent = t(word ? "demo.unchanged" : "demo.typeWord");
}
async function preview() {
  if (!native) return;
  const request = ++previewRequest;
  const word = element<HTMLInputElement>("test-word").value;
  const layout = element<HTMLSelectElement>("test-layout").value as Layout;
  try {
    const result = await invoke<Candidate | null>("preview_word", { word, layout });
    if (request !== previewRequest) return;
    lastPreview = { word, result };
    renderPreview();
  } catch (cause) { if (request === previewRequest) error(String(cause)); }
}
for (const id of ["test-word", "test-layout"]) element(id).addEventListener("input", () => { clearTimeout(previewTimer); previewTimer = setTimeout(() => { void preview(); }, 180); });

// Interface language and appearance. A per-device cache paints the first frame
// correctly before settings arrive from the app.
const darkMedia = window.matchMedia("(prefers-color-scheme: dark)");
let appliedLanguage = "";
let appliedTheme = "";
let reportedInterface = "";
let cachedSignature = "";
function cachedInterface(): { language: LanguageSetting; theme: ThemeSetting } {
  try {
    const value = JSON.parse(localStorage.getItem("codeswitch.interface") ?? "{}") as Partial<Settings>;
    return { language: (["system", "en", "ru"] as const).find((item) => item === value.language) ?? "system", theme: (["system", "light", "dark"] as const).find((item) => item === value.theme) ?? "system" };
  } catch { return { language: "system", theme: "system" }; }
}
function applyInterface(languageSetting: LanguageSetting, theme: ThemeSetting) {
  const resolved = resolveLanguage(languageSetting);
  if (resolved !== appliedLanguage) {
    appliedLanguage = resolved;
    setLanguage(resolved);
    translateDocument();
    showPage(currentPage);
    renderPreview();
  }
  const resolvedTheme = theme === "system" ? (darkMedia.matches ? "dark" : "light") : theme;
  document.documentElement.dataset.theme = resolvedTheme;
  document.documentElement.style.colorScheme = resolvedTheme;
  if (theme !== appliedTheme) {
    appliedTheme = theme;
    // Match the native title bar; null follows the system again.
    if (native) void getCurrentWindow().setTheme(theme === "system" ? null : theme).catch(() => undefined);
  }
  const cache = JSON.stringify({ language: languageSetting, theme });
  if (cache !== cachedSignature) { cachedSignature = cache; try { localStorage.setItem("codeswitch.interface", cache); } catch { /* cache only */ } }
}
function renderInterface() {
  const { language: languageSetting, theme } = status.settings;
  applyInterface(languageSetting, theme);
  for (const [id, value] of [["language-choice", languageSetting], ["theme-choice", theme]] as const) {
    element(id).querySelectorAll<HTMLButtonElement>("button").forEach((button) => {
      button.setAttribute("aria-checked", String(button.dataset.value === value));
      button.disabled = !native || saving;
    });
  }
}
async function setInterface(changes: Partial<Pick<Settings, "language" | "theme">>) {
  const next = { language: status.settings.language, theme: status.settings.theme, ...changes };
  const previous = { language: status.settings.language, theme: status.settings.theme };
  status.settings = { ...status.settings, ...next };
  renderInterface();
  if (!native) return;
  try { await reportInterface(true); error(null); }
  catch (cause) { status.settings = { ...status.settings, ...previous }; renderInterface(); error(String(cause)); }
}
/** Save the interface choice and tell the app which language the tray menu should use. */
async function reportInterface(force = false) {
  const { language: languageSetting, theme } = status.settings;
  const signature = `${languageSetting}/${theme}/${resolveLanguage(languageSetting)}`;
  if (!native || (!force && signature === reportedInterface)) return;
  await invoke("set_interface", { language: languageSetting, theme, russian: resolveLanguage(languageSetting) === "ru" });
  reportedInterface = signature;
}
element("language-choice").addEventListener("click", (event) => {
  const value = (event.target as HTMLElement).closest<HTMLButtonElement>("button")?.dataset.value as LanguageSetting | undefined;
  if (value && value !== status.settings.language) void setInterface({ language: value });
});
element("theme-choice").addEventListener("click", (event) => {
  const value = (event.target as HTMLElement).closest<HTMLButtonElement>("button")?.dataset.value as ThemeSetting | undefined;
  if (value && value !== status.settings.theme) void setInterface({ theme: value });
});
darkMedia.addEventListener("change", () => applyInterface(status.settings.language, status.settings.theme));
window.addEventListener("languagechange", () => { appliedLanguage = ""; renderInterface(); void reportInterface().catch(() => undefined); });

status.settings = { ...status.settings, ...cachedInterface() };
render();
renderPreview();
void refresh().then(() => { void reportInterface().catch(() => undefined); return preview(); });
setInterval(() => { if (!document.hidden) void refresh(); }, 1500);
