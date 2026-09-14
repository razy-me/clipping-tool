import './style.css';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';

// ── Types ─────────────────────────────────────────────────────────────────────
type BufferState = 'stopped' | 'starting' | 'active' | 'paused' | 'error';

interface HotkeyBinding {
  id: string;
  hotkey: string;
  duration_secs: number; // 0 = full buffer
  label: string;
}

interface CleanupConfig {
  enabled: boolean;
  max_age_days: number;
  max_storage_gb: number;
  min_free_disk_gb: number;
}

interface AppConfig {
  custom_clip_path: string;
  buffer_length_secs: number;
  fps_selection: string;
  video_resolution: string;
  bitrate_preset: string;   // 'Low' | 'Balanced' | 'High' | 'Ultra'
  video_codec?: 'H264' | 'HEVC' | 'AV1';
  mic_threshold: number;
  mic_volume?: number;
  hotkey: string;
  hotkeys?: HotkeyBinding[];
  autostart: boolean;
  auto_clipboard?: boolean;
  spike_detection_enabled?: boolean;
  spike_threshold?: number;
  isolated_apps: string[];
  show_cursor_in_clips?: boolean;
  monitor_idx?: number;
  controller_clipping?: boolean;
  audio_sync_offset_ms?: number;
  cleanup?: CleanupConfig;
  hdr_tonemapping?: boolean;
  auto_pause_idle?: boolean;
  auto_pause_idle_minutes?: number;
  overlay_notification?: boolean;
}

interface MonitorInfo {
  index: number;
  name: string;
  width: number;
  height: number;
  is_primary: boolean;
}

interface DiskSpaceInfo {
  total_gb: number;
  free_gb: number;
  used_gb: number;
  is_low_space: boolean;
}

interface AudioTrackInfo {
  track_index: number;
  process_name: string;
  wav_filename?: string;
}

interface ClipItem {
  id: string;
  filename: string;
  full_path: string;
  game_tag: string;
  duration_secs: number;
  created_at: string;
  favorite?: boolean;
  preview_path?: string | null;
  audio_tracks: AudioTrackInfo[];
  spike_markers?: number[];
}

type ViewName = 'home' | 'library' | 'performance' | 'settings';

const $id = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

let cfg: AppConfig;
let appWindow: ReturnType<typeof getCurrentWindow> | null = null;

let bufferState: BufferState = 'stopped';
let activeSince: number | null = null;
let bufferTicker: number | null = null;
let micTimer: number | null = null;
let splitTimer: number | null = null;
let perfTimer: number | null = null;
let libraryCache: ClipItem[] = [];
let libraryLoaded = false;
let currentView: ViewName = 'home';

// ── Helpers ───────────────────────────────────────────────────────────────────
function toast(msg: string, kind: 'success' | 'error' | 'info' = 'info'): void {
  let host = document.getElementById('toast-host');
  if (!host) {
    host = document.createElement('div');
    host.id = 'toast-host';
    document.body.appendChild(host);
  }
  const el = document.createElement('div');
  el.className = 'toast toast-' + kind;

  const msgSpan = document.createElement('span');
  msgSpan.className = 'toast-msg';
  msgSpan.textContent = msg;
  el.appendChild(msgSpan);

  if (kind === 'error') {
    const copyBtn = document.createElement('button');
    copyBtn.className = 'toast-btn-copy';
    copyBtn.innerHTML = svgIcon('copy') + ' <span>Kopieren</span>';
    copyBtn.title = 'Fehlermeldung kopieren';
    copyBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        await navigator.clipboard.writeText(msg);
        copyBtn.classList.add('copied');
        copyBtn.innerHTML = '✓ <span>Kopiert!</span>';
        setTimeout(() => {
          copyBtn.classList.remove('copied');
          copyBtn.innerHTML = svgIcon('copy') + ' <span>Kopieren</span>';
        }, 2000);
      } catch {
        // Fallback
      }
    });
    el.appendChild(copyBtn);
  }

  host.appendChild(el);
  el.style.cursor = 'pointer';
  el.title = 'Klicken zum Schließen';
  el.addEventListener('click', (e) => {
    if ((e.target as HTMLElement).tagName === 'BUTTON') return;
    el.classList.remove('show');
    setTimeout(() => el.remove(), 150);
  });

  requestAnimationFrame(() => el.classList.add('show'));
  const duration = kind === 'error' ? 8000 : 4200;
  setTimeout(() => {
    el.classList.remove('show');
    setTimeout(() => el.remove(), 260);
  }, duration);
}

function fmtDate(iso: string): string {
  try {
    return new Date(iso).toLocaleString(undefined, {
      month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    });
  } catch { return iso.slice(0, 16); }
}

function fmtDur(s: number): string {
  const total = Math.round(Math.max(0, s));
  const m = Math.floor(total / 60);
  const sec = total % 60;
  return m + ':' + String(sec).padStart(2, '0');
}

function esc(s: string): string {
  return (s || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function svgIcon(name: string): string {
  const paths: Record<string, string> = {
    play: '<path d="M8 5.5v13l11-6.5-11-6.5z" fill="currentColor" stroke="none"/>',
    star: '<path d="m12 3 2.7 5.6 6.1.8-4.4 4.3 1.1 6-5.5-3-5.5 3 1.1-6L3.2 9.4l6.1-.8L12 3z"/>',
    trash: '<path d="M4 7h16M9 7V5a1.5 1.5 0 0 1 1.5-1.5h3A1.5 1.5 0 0 1 15 5v2m3 0-.8 12.2A2 2 0 0 1 15.2 21H8.8a2 2 0 0 1-2-1.8L6 7"/>',
    edit: '<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/>',
    folder: '<path d="M3 7V5.5A1.5 1.5 0 0 1 4.5 4h5L12 6.5h7.5A1.5 1.5 0 0 1 21 8v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/>',
    copy: '<path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>',
    refresh: '<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.85.83 6.72 2.24L21 7"/><path d="M21 3v4h-4"/>',
    pause: '<rect x="6" y="5" width="4" height="14" rx="1" fill="currentColor"/><rect x="14" y="5" width="4" height="14" rx="1" fill="currentColor"/>',
    stop: '<rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor"/>',
    close: '<path d="M18 6 6 18M6 6l12 12"/>',
  };
  return '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">' + (paths[name] ?? '') + '</svg>';
}

// ── Buffer State Management ───────────────────────────────────────────────────
let lastErrorDetail = '';

async function copyDiagnosticReport(): Promise<void> {
  try {
    const report = await invoke<string>('get_last_error_log');
    const textToCopy = report && report.trim() ? report : 'Puffer-Status: ' + bufferState + (lastErrorDetail ? '\nFehler: ' + lastErrorDetail : '');
    await navigator.clipboard.writeText(textToCopy);
    toast('Fehlerbericht in die Zwischenablage kopiert! 📋', 'success');
  } catch (e) {
    toast('Kopieren fehlgeschlagen: ' + String(e), 'error');
  }
}

function updateBufferState(state: BufferState, detail = ''): void {
  bufferState = state;
  lastErrorDetail = detail;

  if (state === 'active' && activeSince === null) {
    activeSince = Date.now();
  } else if (state === 'stopped' || state === 'error') {
    activeSince = null;
  }

  // Update top titlebar status pill
  const topPill = $id('top-status-indicator');
  const topText = $id('top-status-text');
  if (topPill && topText) {
    topPill.className = 'top-status-pill pill-' + state;
    if (state === 'active') {
      topText.textContent = 'Aufnahme aktiv';
      topPill.title = '';
    } else if (state === 'paused') {
      topText.textContent = 'Puffer pausiert';
      topPill.title = '';
    } else if (state === 'starting') {
      topText.textContent = 'Initialisiere…';
      topPill.title = '';
    } else if (state === 'error') {
      topText.textContent = '⚠️ ' + (detail ? detail : 'Encoder-Fehler') + ' (Klicken zum Kopieren)';
      topPill.title = 'Klicken um den vollständigen Fehlerbericht zu kopieren';
    } else {
      topText.textContent = 'Puffer inaktiv';
      topPill.title = '';
    }
  }

  // Update sidebar mini widget
  const sStatus = $id('s-buf-status');
  const sDot = document.querySelector('.s-buf-dot') as HTMLElement | null;
  if (sStatus) {
    sStatus.textContent = state === 'active' ? 'Aktiv' : state === 'paused' ? 'Pausiert' : state === 'error' ? 'Fehler' : 'Inaktiv';
  }
  if (sDot) {
    sDot.style.background = state === 'active' ? 'var(--success)' : state === 'paused' ? 'var(--warn)' : state === 'error' ? 'var(--danger)' : 'var(--text-dim)';
  }

  // Refresh home view controls if currently visible
  if (currentView === 'home') {
    renderHomeBufferControls();
    renderHomeErrorBanner();
  }
}

async function startBuffer(): Promise<void> {
  try {
    await invoke('start_buffer');
  } catch (e) {
    toast(String(e), 'error');
  }
}

async function pauseBuffer(): Promise<void> {
  try {
    await invoke('pause_buffer');
    toast('Puffer pausiert', 'info');
  } catch (e) {
    toast(String(e), 'error');
  }
}

async function resumeBuffer(): Promise<void> {
  try {
    await invoke('resume_buffer');
    toast('Puffer fortgesetzt', 'success');
  } catch (e) {
    toast(String(e), 'error');
  }
}

async function stopBuffer(): Promise<void> {
  try {
    await invoke('stop_buffer');
    toast('Puffer gestoppt', 'info');
  } catch (e) {
    toast(String(e), 'error');
  }
}

async function restartIfRunning(): Promise<void> {
  try {
    const st = await invoke<string>('get_buffer_state');
    if (st === 'active' || st === 'starting') {
      await stopBuffer();
      await startBuffer();
    }
  } catch { /* ignore */ }
}

// ── Buffer Progress Ticker ────────────────────────────────────────────────────
function startBufferTicker(): void {
  if (bufferTicker !== null) clearInterval(bufferTicker);
  bufferTicker = setInterval(() => {
    if (document.visibilityState === 'hidden' || !cfg) return;

    const maxLen = Math.max(1, cfg.buffer_length_secs);
    let currentSecs = 0;
    if (bufferState === 'active' && activeSince !== null) {
      currentSecs = Math.min(maxLen, Math.floor((Date.now() - activeSince) / 1000));
    }

    // Update big save button with currently buffered seconds
    const btnLabel = $id('hero-save-btn-text');
    if (btnLabel) {
      if (bufferState === 'active') {
        btnLabel.innerHTML = `Letzte <b>${currentSecs}s</b> speichern`;
      } else if (bufferState === 'paused') {
        btnLabel.innerHTML = `Letzte <b>${currentSecs}s</b> speichern <span style="font-size:0.8em;opacity:0.85">(Pausiert)</span>`;
      } else {
        btnLabel.innerHTML = `Letzte <b>${maxLen}s</b> speichern`;
      }
    }
  }, 300);
}

// ── Sidebar Collapse Controller ───────────────────────────────────────────────
function initSidebar(): void {
  const sidebar = $id('sidebar');
  const toggleBtn = $id('sidebar-toggle');
  const isCollapsed = localStorage.getItem('cliptool_sidebar_collapsed') === 'true';

  if (isCollapsed) sidebar.classList.add('collapsed');

  toggleBtn.addEventListener('click', () => {
    const willCollapse = !sidebar.classList.contains('collapsed');
    sidebar.classList.toggle('collapsed', willCollapse);
    localStorage.setItem('cliptool_sidebar_collapsed', String(willCollapse));
  });

  document.querySelectorAll<HTMLButtonElement>('.nav-item').forEach((btn) => {
    btn.addEventListener('click', () => {
      const view = btn.dataset.view as ViewName;
      if (view) showView(view);
    });
  });
}

// ── Views Routing ─────────────────────────────────────────────────────────────
function showView(v: ViewName): void {
  currentView = v;
  document.querySelectorAll('.nav-item').forEach((b) =>
    b.classList.toggle('active', (b as HTMLElement).dataset.view === v));

  if (micTimer !== null) { clearInterval(micTimer); micTimer = null; }
  if (splitTimer !== null) { clearInterval(splitTimer); splitTimer = null; }
  if (perfTimer !== null) { clearInterval(perfTimer); perfTimer = null; }

  const app = $id('app');
  app.innerHTML = '';
  if (v === 'home') { renderHome(app); }
  else if (v === 'library') { void renderLibrary(app); }
  else if (v === 'performance') { renderPerformance(app); }
  else { renderSettings(app); }
}

// ── Dashboard (Home View) ─────────────────────────────────────────────────────
function renderHome(root: HTMLElement): void {
  const maxLen = cfg?.buffer_length_secs || 120;
  let initSecs = maxLen;
  if (bufferState === 'active' && activeSince !== null) {
    initSecs = Math.min(maxLen, Math.floor((Date.now() - activeSince) / 1000));
  }

  root.innerHTML =
    '<div class="view" id="view-home">' +
      '<div id="home-error-banner-slot"></div>' +
      '<div class="hero-card">' +
        '<div class="hero-main-row">' +
          '<div class="hero-action-group">' +
            '<button class="btn-hero-save" id="btn-save-clip">' +
              svgIcon('play') +
              '<span id="hero-save-btn-text">Letzte <b>' + (bufferState === 'active' ? initSecs : maxLen) + 's</b> speichern</span>' +
            '</button>' +
            '<kbd id="hero-hotkey">' + esc(cfg.hotkeys?.[0]?.hotkey || cfg.hotkey || 'unset') + '</kbd>' +
          '</div>' +
          '<div class="hero-right-group">' +
            '<div class="strip-encoder-badge" id="dash-encoder">GPU auto-detect</div>' +
            '<div class="buffer-controls-group" id="hero-buf-controls"></div>' +
          '</div>' +
        '</div>' +
      '</div>' +

      '<div class="dashboard-grid-2">' +
        '<div class="card">' +
          '<div class="card-title">Mikrofon Eingang</div>' +
          '<div class="mic-meter-box">' +
            '<div class="mic-vu-shell"><div id="mic-meter-fill"></div></div>' +
            '<div class="mic-vu-meta">' +
              '<span class="mic-dev-name" id="mic-dev-info">Echtzeit-Pegel</span>' +
              '<span id="mic-meter-val" style="font-variant-numeric:tabular-nums;font-weight:700">—</span>' +
            '</div>' +
          '</div>' +
        '</div>' +

        '<div class="card">' +
          '<div class="card-title">Aktive Audio-Spuren</div>' +
          '<div class="chips-row" id="split-chips">' +
            '<span class="chip">Lade Anwendungen…</span>' +
          '</div>' +
        '</div>' +
      '</div>' +

      '<div class="card">' +
        '<div class="view-head" style="margin-bottom:0.75rem">' +
          '<div class="card-title" style="margin-bottom:0">Zuletzt gespeicherte Clips</div>' +
          '<button class="ghost" id="goto-lib-btn" style="font-size:0.76rem;padding:0.25rem 0.65rem">Alle Clips ansehen →</button>' +
        '</div>' +
        '<div class="clips-grid" id="recent-clips-grid"></div>' +
      '</div>' +
    '</div>';

  renderHomeBufferControls();
  renderHomeErrorBanner();

  // Save clip action
  const saveBtn = $id<HTMLButtonElement>('btn-save-clip');
  saveBtn.addEventListener('click', async () => {
    const origHtml = saveBtn.innerHTML;
    saveBtn.disabled = true;
    saveBtn.innerHTML = '<span>Speichere Clip…</span>';
    try {
      await invoke('save_clip_now');
    } catch (e) {
      toast(String(e), 'error');
    } finally {
      saveBtn.disabled = false;
      saveBtn.innerHTML = origHtml;
    }
  });

  $id('goto-lib-btn')?.addEventListener('click', () => showView('library'));

  // Load encoder and scaling method badges
  void Promise.all([
    invoke<string>('get_encoder').catch(() => 'unknown'),
    invoke<string>('get_scaling_method').catch(() => 'CpuFallback'),
  ]).then(([enc, scaling]) => {
    const el = $id('dash-encoder');
    if (el) {
      let encLabel = '<span style="color:#f59e0b">🟡 CPU (' + esc(enc) + ')</span>';
      if (enc.includes('nvenc')) encLabel = '<span style="color:#10b981">🟢 NVIDIA NVENC</span>';
      else if (enc.includes('amf')) encLabel = '<span style="color:#10b981">🟢 AMD AMF</span>';
      else if (enc.includes('qsv')) encLabel = '<span style="color:#10b981">🟢 Intel QSV</span>';

      let scaleBadge = '';
      if (scaling === 'Cuda') scaleBadge = ' <span style="font-size:0.72rem;font-weight:600;color:#10b981;background:rgba(16,185,129,0.12);padding:2px 7px;border-radius:4px;border:1px solid rgba(16,185,129,0.25)">Pure VRAM (CUDA)</span>';
      else if (scaling === 'Qsv') scaleBadge = ' <span style="font-size:0.72rem;font-weight:600;color:#10b981;background:rgba(16,185,129,0.12);padding:2px 7px;border-radius:4px;border:1px solid rgba(16,185,129,0.25)">Pure VRAM (QSV)</span>';
      else if (scaling === 'D3d11Direct') scaleBadge = ' <span style="font-size:0.72rem;font-weight:600;color:#10b981;background:rgba(16,185,129,0.12);padding:2px 7px;border-radius:4px;border:1px solid rgba(16,185,129,0.25)">Direct D3D11</span>';

      el.innerHTML = encLabel + scaleBadge;
    }
  });

  // Load active mic device name
  void invoke<string>('get_active_mic_device').then((name) => {
    const el = $id('mic-dev-info');
    if (el && name) el.textContent = name;
  }).catch(() => {});

  // Live microphone VU-meter polling (120ms with dynamic level color)
  micTimer = setInterval(async () => {
    if (document.visibilityState === 'hidden') return;
    try {
      const lvl = await invoke<number>('get_mic_level');
      const pct = lvl > 0.0001 ? Math.max(0, Math.min(100, Math.pow(lvl, 0.45) * 100)) : 0;
      const fill = $id<HTMLElement>('mic-meter-fill');
      const val = $id<HTMLElement>('mic-meter-val');
      if (fill) {
        fill.style.width = pct.toFixed(1) + '%';
        // Green under normal conditions, yellow when high, red only when peaking/clipping
        if (pct >= 88) {
          fill.style.background = '#ef4444';
        } else if (pct >= 70) {
          fill.style.background = '#f59e0b';
        } else {
          fill.style.background = '#10b981';
        }
      }
      if (val) val.textContent = pct < 2 ? '0%' : pct.toFixed(0) + '%';
    } catch { /* ignore */ }
  }, 120);

  // Active split apps polling
  refreshSplitChips('split-chips');
  splitTimer = setInterval(() => {
    if (document.visibilityState === 'hidden') return;
    refreshSplitChips('split-chips');
  }, 4000);

  loadRecents();
}

function renderHomeErrorBanner(): void {
  const container = $id('home-error-banner-slot');
  if (!container) return;

  if (bufferState === 'error') {
    container.innerHTML =
      '<div class="dashboard-error-banner" style="position:relative;z-index:20">' +
        '<div class="error-banner-head">' +
          '<div class="error-banner-title">' +
            '<span>⚠️ Puffer-Start fehlgeschlagen</span>' +
          '</div>' +
          '<div style="display:flex;gap:0.5rem;align-items:center">' +
            '<button class="btn-copy-banner" id="btn-copy-err-banner" style="flex-shrink:0">' + svgIcon('copy') + ' Fehlerbericht kopieren</button>' +
            '<button class="ghost" id="btn-toggle-raw-log" style="font-size:0.75rem;padding:0.35rem 0.6rem;flex-shrink:0">Log anzeigen ▼</button>' +
          '</div>' +
        '</div>' +
        '<div class="error-banner-msg" style="max-height:90px;overflow-y:auto">' + esc(lastErrorDetail || 'Puffer-Encoder unerwartet beendet') + '</div>' +
        '<pre id="raw-log-block" class="error-banner-msg" style="display:none;margin-top:0.3rem;max-height:180px;overflow-y:auto"></pre>' +
      '</div>';

    $id('btn-copy-err-banner')?.addEventListener('click', () => void copyDiagnosticReport());
    $id('btn-toggle-raw-log')?.addEventListener('click', async () => {
      const block = $id('raw-log-block');
      const toggle = $id('btn-toggle-raw-log');
      if (!block) return;
      if (block.style.display === 'none') {
        const log = await invoke<string>('get_last_error_log').catch(() => 'Kein Log');
        block.textContent = log;
        block.style.display = 'block';
        if (toggle) toggle.textContent = 'Log ausblenden ▲';
      } else {
        block.style.display = 'none';
        if (toggle) toggle.textContent = 'Log anzeigen ▼';
      }
    });
  } else {
    container.innerHTML = '';
  }
}

function renderHomeBufferControls(): void {
  const box = $id('hero-buf-controls');
  if (!box) return;

  if (bufferState === 'active') {
    box.innerHTML =
      '<button class="btn-ctrl btn-pause" id="btn-pause-buf">' + svgIcon('pause') + ' Pausieren</button>' +
      '<button class="btn-ctrl btn-stop" id="btn-stop-buf">' + svgIcon('stop') + ' Stoppen</button>';
  } else if (bufferState === 'paused') {
    box.innerHTML =
      '<button class="btn-ctrl btn-start" id="btn-resume-buf">' + svgIcon('play') + ' Fortsetzen</button>' +
      '<button class="btn-ctrl btn-stop" id="btn-stop-buf">' + svgIcon('stop') + ' Stoppen</button>';
  } else if (bufferState === 'error') {
    box.innerHTML =
      '<button class="btn-ctrl btn-start" id="btn-start-buf">' + svgIcon('play') + ' Erneut versuchen</button>' +
      '<button class="btn-ctrl btn-copy-err" id="btn-copy-error" title="Kopiert die genaue Fehlermeldung & FFmpeg-Logs">' + svgIcon('copy') + ' Fehler kopieren</button>';
  } else {
    box.innerHTML =
      '<button class="btn-ctrl btn-start" id="btn-start-buf">' + svgIcon('play') + ' Puffer starten</button>';
  }

  $id('btn-pause-buf')?.addEventListener('click', () => void pauseBuffer());
  $id('btn-resume-buf')?.addEventListener('click', () => void resumeBuffer());
  $id('btn-stop-buf')?.addEventListener('click', () => void stopBuffer());
  $id('btn-start-buf')?.addEventListener('click', () => void startBuffer());
  $id('btn-copy-error')?.addEventListener('click', () => void copyDiagnosticReport());
}

async function refreshSplitChips(containerId: string): Promise<void> {
  try {
    const names = await invoke<string[]>('get_active_split_apps');
    const box = document.getElementById(containerId);
    if (!box) return;
    if (names.length === 0) {
      box.innerHTML = '<span class="chip" style="color:var(--text-dim)">Warte auf Audio-Ausgabe…</span>';
      return;
    }
    box.innerHTML = names.map((n) =>
      '<span class="chip chip-active" style="cursor:default"><span class="live-dot"></span>' + esc(n) + '</span>'
    ).join('');
  } catch { /* ignore */ }
}

async function loadRecents(): Promise<void> {
  if (!libraryLoaded) await loadLibrary();
  const row = $id('recent-clips-grid');
  if (!row) return;
  if (libraryCache.length === 0) {
    row.innerHTML = '<div class="empty-state" style="grid-column:1/-1">Noch keine Clips vorhanden.<br/>Drücke ' + esc(cfg.hotkey || 'den Hotkey') + ' um deinen ersten Clip zu speichern.</div>';
    return;
  }
  row.innerHTML = libraryCache.slice(0, 4).map(cardHtml).join('');
  wireCards(row);
}

// ── Library (Clips View) ──────────────────────────────────────────────────────
let libSearch = '', libSort: 'new' | 'old' | 'dur' = 'new', libFavOnly = false, libGame = '';

async function loadLibrary(): Promise<void> {
  try {
    libraryCache = await invoke<ClipItem[]>('get_all_clips');
    const badge = $id('nav-clips-badge');
    if (badge) {
      badge.textContent = String(libraryCache.length);
      badge.style.display = libraryCache.length > 0 ? 'inline-block' : 'none';
    }
  } catch (e) {
    libraryCache = [];
    toast(String(e), 'error');
  }
  libraryLoaded = true;
}

function cardHtml(c: ClipItem): string {
  const thumb = c.preview_path
    ? '<img class="clip-thumbnail" loading="lazy" src="' + convertFileSrc(c.preview_path) + '" alt=""/>'
    : '<div class="clip-thumbnail" style="display:grid;place-items:center;color:#334155"><svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="4.5" width="18" height="15" rx="2.5"/><path d="M10 9.3v5.4L14.6 12 10 9.3z"/></svg></div>';
  const nSpikes = c.spike_markers ? c.spike_markers.length : 0;

  return (
    '<article class="clip-card" data-id="' + c.id + '" data-path="' + esc(c.full_path) + '" title="Klick: Vorschau abspielen · Doppelklick: Im Web-Editor bearbeiten">' +
      '<div class="clip-thumb-wrap">' +
        thumb +
        '<button class="card-fav-btn ' + (c.favorite ? 'is-fav' : '') + '" title="Favorit umschalten" aria-label="Favorit">' +
          svgIcon('star') +
        '</button>' +
        '<button class="card-copy-btn" title="In Zwischenablage kopieren (Strg+V)" aria-label="Kopieren">📋</button>' +
        '<button class="card-del-btn" title="Clip löschen" aria-label="Löschen">' +
          svgIcon('trash') +
        '</button>' +
        (nSpikes > 0 ? '<span class="clip-spike-badge" title="' + nSpikes + ' Hype Peak(s)">🔥 ' + nSpikes + '</span>' : '') +
        '<span class="clip-dur-badge">' + fmtDur(c.duration_secs) + '</span>' +
      '</div>' +
      '<div class="clip-info">' +
        '<div class="clip-title">' + esc(c.filename) + '</div>' +
        '<div class="clip-meta">' +
          '<span class="clip-game-tag">' + esc(c.game_tag || 'Clip') + '</span>' +
          '<span class="clip-date">' + fmtDate(c.created_at) + '</span>' +
        '</div>' +
      '</div>' +
    '</article>'
  );
}

function filteredLibrary(): ClipItem[] {
  let out = [...libraryCache];
  if (libFavOnly) out = out.filter((c) => c.favorite);
  if (libGame) out = out.filter((c) => c.game_tag === libGame);
  if (libSearch) {
    const q = libSearch.toLowerCase();
    out = out.filter((c) => c.filename.toLowerCase().includes(q) || c.game_tag.toLowerCase().includes(q));
  }
  if (libSort === 'new') out.sort((a, b) => b.created_at.localeCompare(a.created_at));
  if (libSort === 'old') out.sort((a, b) => a.created_at.localeCompare(b.created_at));
  if (libSort === 'dur') out.sort((a, b) => b.duration_secs - a.duration_secs);
  return out;
}

async function renderLibrary(root: HTMLElement): Promise<void> {
  root.innerHTML =
    '<div class="view" id="view-library">' +
      '<div class="view-head">' +
        '<div class="view-title-group">' +
          '<h1>Clip-Bibliothek</h1>' +
          '<div style="display:flex;gap:0.75rem;align-items:center">' +
            '<span class="view-sub" id="lib-count">Lade Clips…</span>' +
            '<span class="view-sub" id="lib-disk-space" style="font-size:0.78rem;font-weight:600;color:var(--text-muted);background:rgba(255,255,255,0.05);padding:2px 8px;border-radius:6px">💾 Lade Speicher…</span>' +
          '</div>' +
        '</div>' +
      '</div>' +
      '<div class="lib-toolbar">' +
        '<input type="text" class="lib-search-input" id="lib-search" placeholder="Clips durchsuchen…" />' +
        '<select class="lib-select" id="lib-sort">' +
          '<option value="new">Neueste zuerst</option>' +
          '<option value="old">Älteste zuerst</option>' +
          '<option value="dur">Längste zuerst</option>' +
        '</select>' +
        '<button class="chip ' + (libFavOnly ? 'chip-active' : '') + '" id="lib-fav-chip">★ Nur Favoriten</button>' +
        '<button class="chip" id="lib-refresh-btn" title="Aktualisieren">' + svgIcon('refresh') + ' Aktualisieren</button>' +
        '<div class="chips-row" id="lib-games" style="margin-left:auto"></div>' +
      '</div>' +
      '<div id="lib-body"></div>' +
    '</div>';

  const search = $id<HTMLInputElement>('lib-search');
  search.value = libSearch;
  search.addEventListener('input', () => { libSearch = search.value; paintLibrary(); });

  const sort = $id<HTMLSelectElement>('lib-sort');
  sort.value = libSort;
  sort.addEventListener('change', () => { libSort = sort.value as typeof libSort; paintLibrary(); });

  const favChip = $id<HTMLButtonElement>('lib-fav-chip');
  favChip.addEventListener('click', () => {
    libFavOnly = !libFavOnly;
    favChip.classList.toggle('chip-active', libFavOnly);
    paintLibrary();
  });

  const refreshBtn = $id<HTMLButtonElement>('lib-refresh-btn');
  refreshBtn?.addEventListener('click', async () => {
    await loadLibrary();
    paintLibrary();
    toast('Bibliothek aktualisiert', 'info');
  });

  // Load disk space info
  void invoke<DiskSpaceInfo>('get_disk_space_info').then((ds) => {
    const el = $id('lib-disk-space');
    if (el) {
      if (ds.is_low_space) {
        el.innerHTML = '<span style="color:#ef4444;font-weight:700">⚠️ Fast voll: ' + ds.free_gb.toFixed(1) + ' GB frei</span>';
      } else {
        el.textContent = '💾 ' + ds.free_gb.toFixed(1) + ' GB frei von ' + ds.total_gb.toFixed(0) + ' GB';
      }
    }
  }).catch(() => {});

  if (!libraryLoaded) {
    const body = $id('lib-body');
    if (body) body.innerHTML = '<div class="empty-state">Lade gespeicherte Clips…</div>';
    await loadLibrary();
  }
  paintLibrary();
}

function paintGamesRow(): void {
  const box = $id('lib-games');
  if (!box) return;
  const games = [...new Set(libraryCache.map((c) => c.game_tag).filter(Boolean))];
  if (games.length <= 1) {
    box.innerHTML = '';
    return;
  }
  box.innerHTML =
    '<button class="chip' + (libGame === '' ? ' chip-active' : '') + '" data-game="">Alle</button>' +
    games.slice(0, 8).map((g) =>
      '<button class="chip' + (libGame === g ? ' chip-active' : '') + '" data-game="' + esc(g) + '">' + esc(g) + '</button>'
    ).join('');

  box.querySelectorAll<HTMLButtonElement>('button[data-game]').forEach((btn) =>
    btn.addEventListener('click', () => {
      libGame = btn.dataset.game ?? '';
      paintLibrary();
    })
  );
}

function paintLibrary(): void {
  const body = $id('lib-body');
  if (!body) return;
  const items = filteredLibrary();
  const countEl = $id('lib-count');
  if (countEl) countEl.textContent = items.length + ' von ' + libraryCache.length + ' Clips';
  paintGamesRow();

  if (items.length === 0) {
    body.innerHTML = '<div class="empty-state">Keine Clips gefunden' + (libSearch || libFavOnly || libGame ? ' für die aktuellen Filter.' : ' vorhanden.<br/>Speichere deinen ersten Clip!') + '</div>';
    return;
  }
  body.innerHTML = '<div class="clips-grid">' + items.map(cardHtml).join('') + '</div>';
  wireCards(body);
}

// ── Card Interaction Wiring (Single-Click = Editor, Double-Click = Preview) ────
function wireCards(scope: HTMLElement): void {
  scope.querySelectorAll<HTMLElement>('.clip-card').forEach((cardEl) => {
    const path = cardEl.dataset.path!;
    let clickTimer: number | null = null;

    // Favorite Star Click
    const favBtn = cardEl.querySelector('.card-fav-btn');
    favBtn?.addEventListener('click', async (e) => {
      e.stopPropagation();
      e.preventDefault();
      try {
        const nowFav = await invoke<boolean>('toggle_favorite', { path });
        favBtn.classList.toggle('is-fav', nowFav);
        const cached = libraryCache.find((c) => c.full_path === path);
        if (cached) cached.favorite = nowFav;
      } catch (err) {
        toast(String(err), 'error');
      }
    });

    // Delete Button Click
    const delBtn = cardEl.querySelector('.card-del-btn');
    delBtn?.addEventListener('click', (e) => {
      e.stopPropagation();
      e.preventDefault();
      confirmModal('Diesen Clip löschen?', 'Die Datei und zugehörige Audiospuren werden in den Papierkorb verschoben.', async () => {
        try {
          await invoke('delete_clip', { path });
          libraryCache = libraryCache.filter((c) => c.full_path !== path);
          paintLibrary();
          if (document.querySelector('#view-home')) loadRecents();
          toast('Clip in Papierkorb verschoben', 'success');
        } catch (err) {
          const errStr = String(err);
          if (errStr.includes("not exist") || errStr.includes("nicht mehr")) {
            libraryCache = libraryCache.filter((c) => c.full_path !== path);
            paintLibrary();
            if (document.querySelector('#view-home')) loadRecents();
            toast('Clip war bereits gelöscht und wurde entfernt', 'info');
          } else {
            toast(errStr, 'error');
          }
        }
      });
    });

    // Quick Copy Button Click
    const copyBtn = cardEl.querySelector('.card-copy-btn');
    copyBtn?.addEventListener('click', async (e) => {
      e.stopPropagation();
      e.preventDefault();
      try {
        await invoke('copy_clip_to_clipboard', { path });
        toast('Clip in Zwischenablage kopiert! 📋 (Strg+V)', 'success');
      } catch (err) {
        toast(String(err), 'error');
      }
    });

    // Single Click (In-App Vorschau) vs Double Click (Web-Editor)
    cardEl.addEventListener('click', (e) => {
      if ((e.target as HTMLElement).closest('.card-fav-btn, .card-del-btn, .card-copy-btn')) return;
      if (clickTimer !== null) {
        clearTimeout(clickTimer);
        clickTimer = null;
      }
      clickTimer = window.setTimeout(() => {
        clickTimer = null;
        const clip = libraryCache.find((c) => c.full_path === path);
        if (clip) {
          openPreviewModal(clip);
        }
      }, 230);
    });

    cardEl.addEventListener('dblclick', (e) => {
      if ((e.target as HTMLElement).closest('.card-fav-btn, .card-del-btn, .card-copy-btn')) return;
      if (clickTimer !== null) {
        clearTimeout(clickTimer);
        clickTimer = null;
      }
      const clip = libraryCache.find((c) => c.full_path === path);
      if (clip) {
        toast('Öffne Web-Editor…', 'info');
        invoke('open_editor_window', { clip }).catch((err) => toast(String(err), 'error'));
      }
    });
  });
}

// ── Quick-Preview Lightbox Modal ──────────────────────────────────────────────
function openPreviewModal(clip: ClipItem): void {
  const host = $id('preview-modal-host');
  if (!host) return;

  const overlay = document.createElement('div');
  overlay.className = 'lightbox-overlay';
  overlay.innerHTML =
    '<div class="lightbox-card" role="dialog" aria-modal="true">' +
      '<div class="lightbox-header">' +
        '<span class="lightbox-title">📹 ' + esc(clip.filename) + '</span>' +
        '<button class="titlebar-button titlebar-close" id="lightbox-close" style="width:32px;height:32px;border-radius:6px">' +
          svgIcon('close') +
        '</button>' +
      '</div>' +
      '<div class="lightbox-video-wrap">' +
        '<video class="lightbox-video" src="' + convertFileSrc(clip.full_path) + '" controls autoplay></video>' +
      '</div>' +
      '<div class="lightbox-footer">' +
        '<div class="lightbox-meta">' +
          '<span>Spiel: <b>' + esc(clip.game_tag || 'Clip') + '</b></span> · ' +
          '<span>Dauer: <b>' + fmtDur(clip.duration_secs) + '</b></span> · ' +
          '<span>' + fmtDate(clip.created_at) + '</span>' +
        '</div>' +
        '<div class="lightbox-actions">' +
          '<button class="ghost" id="lb-copy-btn">' + svgIcon('copy') + ' In Zwischenablage (Strg+V)</button>' +
          '<button class="primary" id="lb-edit-btn">' + svgIcon('edit') + ' Im Web-Editor öffnen</button>' +
          '<button class="danger" id="lb-del-btn">' + svgIcon('trash') + '</button>' +
        '</div>' +
      '</div>' +
    '</div>';

  host.innerHTML = '';
  host.appendChild(overlay);

  let onKey: ((ev: KeyboardEvent) => void) | null = null;
  const close = () => {
    if (onKey) {
      window.removeEventListener('keydown', onKey);
      onKey = null;
    }
    const vid = overlay.querySelector('video');
    if (vid) vid.pause();
    overlay.remove();
  };

  onKey = (ev: KeyboardEvent) => {
    if (ev.key === 'Escape') close();
  };
  window.addEventListener('keydown', onKey);

  overlay.querySelector('#lightbox-close')?.addEventListener('click', close);
  overlay.addEventListener('click', (ev) => {
    if (ev.target === overlay) close();
  });

  // Actions
  overlay.querySelector('#lb-copy-btn')?.addEventListener('click', async () => {
    try {
      await invoke('copy_clip_to_clipboard', { path: clip.full_path });
      toast('In Zwischenablage kopiert (Strg+V)!', 'success');
    } catch (err) {
      toast(String(err), 'error');
    }
  });

  overlay.querySelector('#lb-edit-btn')?.addEventListener('click', async () => {
    close();
    toast('Öffne Web-Editor…', 'info');
    await invoke('open_editor_window', { clip }).catch((err) => toast(String(err), 'error'));
  });

  overlay.querySelector('#lb-del-btn')?.addEventListener('click', () => {
    confirmModal('Diesen Clip löschen?', 'Die Datei und zugehörige Audiospuren werden in den Papierkorb verschoben.', async () => {
      try {
        await invoke('delete_clip', { path: clip.full_path });
        libraryCache = libraryCache.filter((c) => c.full_path !== clip.full_path);
        close();
        if (document.querySelector('#view-home')) loadRecents();
        if (document.querySelector('#view-library')) paintLibrary();
        toast('Clip gelöscht', 'success');
      } catch (err) {
        toast(String(err), 'error');
      }
    });
  });
}

// ── Confirmation Modal ────────────────────────────────────────────────────────
function confirmModal(title: string, message: string, onConfirm: () => void): void {
  const ov = document.createElement('div');
  ov.className = 'modal-overlay';
  ov.innerHTML =
    '<div class="modal-card" role="dialog" aria-modal="true">' +
      '<h3>' + esc(title) + '</h3>' +
      '<p>' + esc(message) + '</p>' +
      '<div class="modal-actions">' +
        '<button class="ghost modal-no">Abbrechen</button>' +
        '<button class="danger modal-yes">Löschen</button>' +
      '</div>' +
    '</div>';
  document.body.appendChild(ov);

  let onKey: ((ev: KeyboardEvent) => void) | null = null;
  const close = () => {
    if (onKey) {
      window.removeEventListener('keydown', onKey);
      onKey = null;
    }
    ov.remove();
  };

  onKey = (ev: KeyboardEvent) => {
    if (ev.key === 'Escape') close();
  };
  window.addEventListener('keydown', onKey);

  ov.querySelector('.modal-no')?.addEventListener('click', close);
  ov.querySelector('.modal-yes')?.addEventListener('click', () => { close(); onConfirm(); });
  ov.addEventListener('mousedown', (ev) => { if (ev.target === ov) close(); });
}

// ── Performance Monitor View ──────────────────────────────────────────────────
interface ProcessMetric {
  pid: number;
  name: string;
  role: string;
  cpu_usage: number;
  cpu_usage_normalized: number;
  memory_bytes: number;
  memory_mb: number;
  virtual_memory_bytes: number;
  virtual_memory_mb: number;
  disk_read_bytes: number;
  disk_written_bytes: number;
}

interface SystemSpecs {
  os_name: string;
  cpu_name: string;
  cpu_cores: number;
  total_ram_mb: number;
  available_ram_mb: number;
  power_source: string;
  hardware_tier: string;
  gpu_name: string;
  display_resolution: string;
  display_refresh_rate: number;
  capture_pixel_rate_mps: number;
  raw_video_bandwidth_mbs: number;
  disk_free_gb: number;
  disk_total_gb: number;
}

interface ActiveSettingsSnapshot {
  video_codec: string;
  video_resolution: string;
  fps: string;
  bitrate_preset: string;
  buffer_length_secs: number;
  monitor_idx: number;
  show_cursor: boolean;
  hdr_tonemapping: boolean;
  spike_detection: boolean;
  spike_threshold: number;
  controller_clipping: boolean;
  audio_sync_offset_ms: number;
  isolated_apps_count: number;
}

interface PipelineDiagnostics {
  active_encoder: string;
  hw_acceleration: string;
  target_resolution: string;
  pixel_reduction_pct: number;
  video_buffer_stored_secs: number;
  video_buffer_chunks: number;
  audio_tracks_count: number;
  scaling_method: string;
}

interface ToolPerformanceSnapshot {
  timestamp_ms: number;
  cpu_cores: number;
  total_cpu_usage: number;
  total_cpu_normalized: number;
  total_memory_bytes: number;
  total_memory_mb: number;
  total_virtual_memory_bytes: number;
  total_virtual_memory_mb: number;
  video_buffer_bytes: number;
  video_buffer_mb: number;
  video_buffer_max_bytes: number;
  video_buffer_max_mb: number;
  audio_buffer_bytes: number;
  audio_buffer_mb: number;
  system_specs: SystemSpecs;
  active_settings: ActiveSettingsSnapshot;
  pipeline_diagnostics: PipelineDiagnostics;
  bottleneck_warnings: string[];
  processes: ProcessMetric[];
}

interface PerfHistoryPoint {
  timeMs: number;
  cpuNorm: number;
  cpuTotal: number;
  ramMb: number;
  vidBufMb: number;
  audBufMb: number;
  ffmpegCpu: number;
  ffmpegRam: number;
  clipToolCpu: number;
  clipToolRam: number;
  webViewCpu: number;
  webViewRam: number;
  processesCount: number;
}

let perfIntervalMs = 1000;
let perfPaused = false;
let latestSnapshot: ToolPerformanceSnapshot | null = null;
const perfHistory: PerfHistoryPoint[] = [];
const TIME_WINDOW_MS = 60_000; // Immer exakt 60 Sekunden Zeitfenster

function drawPerfCanvas(canvas: HTMLCanvasElement): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return;

  canvas.width = Math.floor(rect.width * dpr);
  canvas.height = Math.floor(rect.height * dpr);
  ctx.scale(dpr, dpr);

  const w = rect.width;
  const h = rect.height;

  ctx.clearRect(0, 0, w, h);

  const chartBottom = h - 16;
  const chartHeight = chartBottom - 18;

  // Background horizontal grid lines
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
  ctx.lineWidth = 1;
  for (let i = 1; i <= 3; i++) {
    const y = Math.floor(18 + (chartHeight / 4) * i);
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
    ctx.stroke();
  }

  // Vertical time grid lines (every 15 seconds)
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
  for (let s = 15; s <= 45; s += 15) {
    const x = Math.floor(w - (s / 60) * w);
    ctx.beginPath();
    ctx.moveTo(x, 16);
    ctx.lineTo(x, chartBottom);
    ctx.stroke();
  }

  // Time labels on bottom axis
  ctx.fillStyle = '#64748b';
  ctx.font = '9px sans-serif';
  ctx.textAlign = 'left';
  ctx.fillText('vor 60s', 4, h - 3);
  ctx.textAlign = 'center';
  ctx.fillText('vor 45s', w * 0.25, h - 3);
  ctx.fillText('vor 30s', w * 0.50, h - 3);
  ctx.fillText('vor 15s', w * 0.75, h - 3);
  ctx.textAlign = 'right';
  ctx.fillText('Jetzt (0s)', w - 4, h - 3);

  if (perfHistory.length < 2) {
    ctx.fillStyle = '#64748b';
    ctx.font = '11px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('Sammle Messwerte…', w / 2, chartBottom / 2 + 8);
    return;
  }

  const now = Date.now();
  // Map timestamp to X position on canvas: right edge = now, left edge = now - 60s
  const getX = (t: number) => {
    const elapsed = now - t;
    const ratio = Math.max(0, Math.min(1, 1 - elapsed / TIME_WINDOW_MS));
    return ratio * w;
  };

  const maxCpu = Math.max(10, ...perfHistory.map((p) => p.cpuNorm));
  const maxRam = Math.max(150, ...perfHistory.map((p) => p.ramMb));

  // 1. Draw RAM Area & Line (Emerald)
  ctx.beginPath();
  perfHistory.forEach((p, idx) => {
    const x = getX(p.timeMs);
    const y = chartBottom - (p.ramMb / maxRam) * chartHeight;
    if (idx === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.strokeStyle = '#10b981';
  ctx.lineWidth = 2;
  ctx.stroke();

  // Fill gradient for RAM
  const firstX = getX(perfHistory[0].timeMs);
  const lastX = getX(perfHistory[perfHistory.length - 1].timeMs);
  const ramGradient = ctx.createLinearGradient(0, 0, 0, chartBottom);
  ramGradient.addColorStop(0, 'rgba(16, 185, 129, 0.20)');
  ramGradient.addColorStop(1, 'rgba(16, 185, 129, 0.0)');
  ctx.lineTo(lastX, chartBottom);
  ctx.lineTo(firstX, chartBottom);
  ctx.closePath();
  ctx.fillStyle = ramGradient;
  ctx.fill();

  // 2. Draw CPU Area & Line (Cyan)
  ctx.beginPath();
  perfHistory.forEach((p, idx) => {
    const x = getX(p.timeMs);
    const y = chartBottom - (p.cpuNorm / maxCpu) * chartHeight;
    if (idx === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.strokeStyle = '#06b6d4';
  ctx.lineWidth = 2;
  ctx.stroke();

  // Fill gradient for CPU
  const cpuGradient = ctx.createLinearGradient(0, 0, 0, chartBottom);
  cpuGradient.addColorStop(0, 'rgba(6, 182, 212, 0.25)');
  cpuGradient.addColorStop(1, 'rgba(6, 182, 212, 0.0)');
  ctx.lineTo(lastX, chartBottom);
  ctx.lineTo(firstX, chartBottom);
  ctx.closePath();
  ctx.fillStyle = cpuGradient;
  ctx.fill();

  // Scale labels (Top left & top right)
  ctx.font = '10px monospace';
  ctx.textAlign = 'left';
  ctx.fillStyle = '#06b6d4';
  ctx.fillText(`CPU Skala: ${maxCpu.toFixed(1)} %`, 8, 12);

  ctx.textAlign = 'right';
  ctx.fillStyle = '#10b981';
  ctx.fillText(`RAM Skala: ${maxRam.toFixed(0)} MB`, w - 8, 12);
}

function renderPerformance(root: HTMLElement): void {
  root.innerHTML =
    '<div class="view" id="view-performance">' +
      '<div class="perf-header-bar">' +
        '<div class="perf-title-group">' +
          '<h1>' +
            '<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="color:var(--accent)">' +
              '<path d="M22 12h-4l-3 9L9 3l-3 9H2"/>' +
            '</svg>' +
            'Performance-Monitor' +
          '</h1>' +
          '<p>Echtzeit-Messung exklusiv für ClipTool und seine Subprozesse (PID-Baum)</p>' +
        '</div>' +
        '<div class="perf-actions-group">' +
          '<div style="font-size:0.78rem;color:var(--text-muted);display:flex;align-items:center;gap:0.4rem">' +
            '<span>Abtastrate:</span>' +
            '<div style="display:inline-flex;align-items:center;gap:0.2rem;background:var(--bg-input, rgba(255,255,255,0.06));border:1px solid var(--border-subtle, rgba(255,255,255,0.12));border-radius:6px;padding:0.18rem 0.45rem">' +
              '<input type="number" id="perf-interval-input" class="perf-input" min="100" max="60000" step="100" value="' + perfIntervalMs + '" placeholder="1000" style="width:64px;background:transparent;border:none;color:var(--text-primary,#fff);font-size:0.8rem;text-align:right;outline:none;" title="Beliebige Abtastrate in Millisekunden (100 - 60.000 ms)" />' +
              '<span style="font-size:0.72rem;color:var(--text-muted);">ms</span>' +
            '</div>' +
            '<select id="perf-interval-select" class="perf-select" style="font-size:0.75rem;padding:0.2rem 0.35rem;border-radius:6px;" title="Vorgefertigte Profile">' +
              '<option value="">⚡ Profile...</option>' +
              '<option value="250"' + (perfIntervalMs === 250 ? ' selected' : '') + '>250 ms (Echtzeit)</option>' +
              '<option value="500"' + (perfIntervalMs === 500 ? ' selected' : '') + '>500 ms (Präzise)</option>' +
              '<option value="1000"' + (perfIntervalMs === 1000 ? ' selected' : '') + '>1.000 ms (Standard)</option>' +
              '<option value="2000"' + (perfIntervalMs === 2000 ? ' selected' : '') + '>2.000 ms (Sparsam)</option>' +
              '<option value="5000"' + (perfIntervalMs === 5000 ? ' selected' : '') + '>5.000 ms (Minimal)</option>' +
            '</select>' +
          '</div>' +
          '<button class="ghost" id="btn-perf-pause" title="Messung pausieren oder fortsetzen">' +
            '<span id="perf-pause-label">' + (perfPaused ? '▶️ Fortsetzen' : '⏸️ Pause') + '</span>' +
          '</button>' +
          '<button class="ghost" id="btn-perf-copy" title="Snapshot in die Zwischenablage kopieren">' +
            '📋 Snapshot kopieren' +
          '</button>' +
          '<button class="primary" id="btn-perf-export-txt" title="Vollständiges 60-Sekunden-Schrittprotokoll als .txt Datei auf Festplatte speichern">' +
            '💾 60s-Snapshot .txt Datei' +
          '</button>' +
        '</div>' +
      '</div>' +

      '<!-- Metric Cards -->' +
      '<div class="perf-cards-grid">' +
        '<div class="perf-card">' +
          '<div class="perf-card-label">💻 Gesamt CPU</div>' +
          '<div class="perf-card-val accent-cpu" id="perf-val-cpu">0.0 %</div>' +
          '<div class="perf-card-sub" id="perf-sub-cpu">Berechne…</div>' +
        '</div>' +
        '<div class="perf-card">' +
          '<div class="perf-card-label">🧠 Gesamt RAM</div>' +
          '<div class="perf-card-val accent-ram" id="perf-val-ram">0.0 MB</div>' +
          '<div class="perf-card-sub" id="perf-sub-ram">Physikalischer Arbeitsspeicher</div>' +
        '</div>' +
        '<div class="perf-card">' +
          '<div class="perf-card-label">📼 Video-Ringpuffer</div>' +
          '<div class="perf-card-val accent-buf" id="perf-val-vidbuf">0.0 MB</div>' +
          '<div class="perf-card-sub" id="perf-sub-vidbuf">In-Memory RAM Puffer</div>' +
        '</div>' +
        '<div class="perf-card">' +
          '<div class="perf-card-label">🎵 Audio-Spuren</div>' +
          '<div class="perf-card-val" id="perf-val-audbuf">0.0 MB</div>' +
          '<div class="perf-card-sub" id="perf-sub-audbuf">System + Mic + Multi-Tracks</div>' +
        '</div>' +
        '<div class="perf-card">' +
          '<div class="perf-card-label">⚡ Subprozesse</div>' +
          '<div class="perf-card-val" id="perf-val-procs">0</div>' +
          '<div class="perf-card-sub">PID-Baum erfasst</div>' +
        '</div>' +
      '</div>' +

      '<!-- 60s Live Graph -->' +
      '<div class="perf-chart-card">' +
        '<div class="perf-chart-head">' +
          '<span>Live-Verlauf (Letzte 60 Sekunden)</span>' +
          '<div class="perf-chart-legend">' +
            '<div class="perf-legend-item"><span class="perf-legend-dot cpu"></span> CPU-Auslastung (%)</div>' +
            '<div class="perf-legend-item"><span class="perf-legend-dot ram"></span> RAM-Verbrauch (MB)</div>' +
          '</div>' +
        '</div>' +
        '<div class="perf-canvas-wrap">' +
          '<canvas id="perf-canvas"></canvas>' +
        '</div>' +
      '</div>' +

      '<!-- Subprocesses Table -->' +
      '<div class="perf-table-card">' +
        '<div class="perf-table-title">' +
          '<span>Detaillierte Subprozess-Aufschlüsselung</span>' +
          '<span style="font-size:0.75rem;font-weight:500;color:var(--text-muted)">Ausschließlich ClipTool-Prozessgruppe</span>' +
        '</div>' +
        '<div class="perf-table-wrap">' +
          '<table class="perf-table">' +
            '<thead>' +
              '<tr>' +
                '<th>Prozess / Komponente</th>' +
                '<th>Rolle & Funktion</th>' +
                '<th>CPU-Last (Gesamt / Core)</th>' +
                '<th>Physikalisch (Working Set)</th>' +
                '<th>Virtuell (Commit)</th>' +
                '<th>I/O Write</th>' +
              '</tr>' +
            '</thead>' +
            '<tbody id="perf-table-body">' +
              '<tr><td colspan="6" style="text-align:center;padding:1.5rem;color:var(--text-dim)">Lade Metriken…</td></tr>' +
            '</tbody>' +
          '</table>' +
        '</div>' +
      '</div>' +

      '<!-- Info Transparency Note -->' +
      '<div class="perf-note-banner">' +
        '<span style="font-size:1.1rem;line-height:1">ℹ️</span>' +
        '<div>' +
          '<b>Haargenaue Prozess-Isolation:</b> Dieser Monitor überwacht mit System-APIs ausschließlich den eigenen Prozessbaum ' +
          '(<code>clip-tool.exe</code>, <code>ffmpeg.exe</code>, <code>msedgewebview2.exe</code>). Spiele, Hintergrund-Apps oder Windows-Dienste ' +
          'werden strikt ausgeblendet, sodass Sie den exakten Netto-Ressourcenverbrauch von ClipTool sehen.' +
        '</div>' +
      '</div>' +
    '</div>';

  // Setup canvas
  const canvas = $id<HTMLCanvasElement>('perf-canvas');
  if (canvas) {
    drawPerfCanvas(canvas);
  }

  // Polling loop
  const runPoll = async () => {
    if (perfPaused || currentView !== 'performance') return;
    try {
      const snap = await invoke<ToolPerformanceSnapshot>('get_tool_performance');
      latestSnapshot = snap;

      // Update Card Values
      const cpuVal = $id('perf-val-cpu');
      const cpuSub = $id('perf-sub-cpu');
      if (cpuVal) cpuVal.textContent = snap.total_cpu_normalized.toFixed(1) + ' %';
      if (cpuSub) cpuSub.textContent = snap.total_cpu_usage.toFixed(1) + ' % Core-Last (' + snap.cpu_cores + ' Kerne)';

      const ramVal = $id('perf-val-ram');
      const ramSub = $id('perf-sub-ram');
      if (ramVal) ramVal.textContent = snap.total_memory_mb.toFixed(1) + ' MB';
      if (ramSub) ramSub.textContent = 'Commit: ' + snap.total_virtual_memory_mb.toFixed(1) + ' MB';

      const vidVal = $id('perf-val-vidbuf');
      const vidSub = $id('perf-sub-vidbuf');
      if (vidVal) vidVal.textContent = snap.video_buffer_mb.toFixed(1) + ' MB';
      if (vidSub) {
        const pct = snap.video_buffer_max_bytes > 0
          ? ((snap.video_buffer_bytes / snap.video_buffer_max_bytes) * 100).toFixed(1)
          : '0.0';
        vidSub.textContent = 'Max: ' + snap.video_buffer_max_mb.toFixed(0) + ' MB (' + pct + ' %)';
      }

      const audVal = $id('perf-val-audbuf');
      if (audVal) audVal.textContent = snap.audio_buffer_mb.toFixed(2) + ' MB';

      const procsVal = $id('perf-val-procs');
      if (procsVal) procsVal.textContent = String(snap.processes.length);

      // Extract key component breakdown for historical step protocol
      let ffmpegCpu = 0;
      let ffmpegRam = 0;
      let clipToolCpu = 0;
      let clipToolRam = 0;
      let webViewCpu = 0;
      let webViewRam = 0;

      for (const p of snap.processes) {
        const nameLower = p.name.toLowerCase();
        if (p.role.includes('FFmpeg') || nameLower.includes('ffmpeg')) {
          ffmpegCpu += p.cpu_usage_normalized;
          ffmpegRam += p.memory_mb;
        } else if (p.role.includes('Hauptanwendung') || nameLower.includes('clip_tool') || nameLower.includes('clip-tool')) {
          clipToolCpu += p.cpu_usage_normalized;
          clipToolRam += p.memory_mb;
        } else if (p.role.includes('WebView2') || nameLower.includes('msedgewebview2')) {
          webViewCpu += p.cpu_usage_normalized;
          webViewRam += p.memory_mb;
        }
      }

      // Add to history with precise timestamp
      const now = Date.now();
      perfHistory.push({
        timeMs: now,
        cpuNorm: snap.total_cpu_normalized,
        cpuTotal: snap.total_cpu_usage,
        ramMb: snap.total_memory_mb,
        vidBufMb: snap.video_buffer_mb,
        audBufMb: snap.audio_buffer_mb,
        ffmpegCpu,
        ffmpegRam,
        clipToolCpu,
        clipToolRam,
        webViewCpu,
        webViewRam,
        processesCount: snap.processes.length,
      });
      // Purge entries older than 60 seconds (with 2s safety buffer)
      while (perfHistory.length > 0 && now - perfHistory[0].timeMs > TIME_WINDOW_MS + 2000) {
        perfHistory.shift();
      }

      // Redraw Canvas
      const curCanvas = $id<HTMLCanvasElement>('perf-canvas');
      if (curCanvas) drawPerfCanvas(curCanvas);

      // Populate Process Table
      const tbody = $id('perf-table-body');
      if (tbody) {
        tbody.innerHTML = snap.processes.map((p) => {
          let badgeClass = 'proc-badge-sub';
          let roleIcon = '⚙️';
          if (p.role.includes('Hauptanwendung')) {
            badgeClass = 'proc-badge-root';
            roleIcon = '⭐';
          } else if (p.role.includes('FFmpeg') || p.role.includes('Aufnahme')) {
            badgeClass = 'proc-badge-ffmpeg';
            roleIcon = '🎥';
          } else if (p.role.includes('WebView2')) {
            badgeClass = 'proc-badge-webview';
            roleIcon = '🌐';
          }

          const writeSpeed = p.disk_written_bytes > 1024 * 1024
            ? (p.disk_written_bytes / (1024 * 1024)).toFixed(1) + ' MB'
            : Math.round(p.disk_written_bytes / 1024) + ' KB';

          return '<tr>' +
            '<td>' +
              '<div style="display:flex;align-items:center;gap:0.4rem;font-weight:650">' +
                '<span class="proc-badge ' + badgeClass + '">' + roleIcon + ' ' + esc(p.name) + '</span>' +
                '<span style="color:var(--text-dim);font-size:0.72rem">(PID ' + p.pid + ')</span>' +
              '</div>' +
            '</td>' +
            '<td style="color:var(--text-muted)">' + esc(p.role) + '</td>' +
            '<td>' +
              '<span style="font-weight:700;color:' + (p.cpu_usage_normalized > 5 ? '#f59e0b' : '#06b6d4') + '">' +
                p.cpu_usage_normalized.toFixed(1) + ' %' +
              '</span>' +
              '<span style="color:var(--text-dim);font-size:0.72rem"> (' + p.cpu_usage.toFixed(1) + '%)</span>' +
            '</td>' +
            '<td style="font-weight:650;color:#10b981">' + p.memory_mb.toFixed(1) + ' MB</td>' +
            '<td style="color:var(--text-muted)">' + p.virtual_memory_mb.toFixed(1) + ' MB</td>' +
            '<td style="color:var(--text-dim)">' + writeSpeed + '</td>' +
          '</tr>';
        }).join('');
      }
    } catch (e) {
      console.error('[Performance] fetch error:', e);
    }
  };

  // Start polling
  const startPolling = () => {
    if (perfTimer !== null) clearInterval(perfTimer);
    void runPoll();
    perfTimer = window.setInterval(() => {
      void runPoll();
    }, perfIntervalMs);
  };
  startPolling();

  // Event Listeners for custom interval input & preset dropdown
  const intervalInput = $id<HTMLInputElement>('perf-interval-input');
  const intervalSelect = $id<HTMLSelectElement>('perf-interval-select');

  const updateInterval = (val: number) => {
    const clamped = Math.max(100, Math.min(60000, Math.round(val)));
    perfIntervalMs = clamped;
    if (intervalInput && document.activeElement !== intervalInput) {
      intervalInput.value = String(clamped);
    }
    if (intervalSelect) {
      const matchingOption = Array.from(intervalSelect.options).find((o) => o.value === String(clamped));
      intervalSelect.value = matchingOption ? String(clamped) : '';
    }
    startPolling();
  };

  intervalInput?.addEventListener('change', (e) => {
    const val = parseInt((e.target as HTMLInputElement).value, 10);
    if (!isNaN(val)) {
      updateInterval(val);
    }
  });

  intervalInput?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      const val = parseInt((e.target as HTMLInputElement).value, 10);
      if (!isNaN(val)) {
        updateInterval(val);
        intervalInput.blur();
      }
    }
  });

  intervalSelect?.addEventListener('change', (e) => {
    const val = parseInt((e.target as HTMLSelectElement).value, 10);
    if (!isNaN(val) && val > 0) {
      if (intervalInput) intervalInput.value = String(val);
      updateInterval(val);
    }
  });

  const btnPause = $id('btn-perf-pause');
  btnPause?.addEventListener('click', () => {
    perfPaused = !perfPaused;
    const label = $id('perf-pause-label');
    if (label) {
      label.textContent = perfPaused ? '▶️ Fortsetzen' : '⏸️ Pause';
    }
    if (!perfPaused) void runPoll();
  });

  function buildPerformanceSnapshotText(snap: ToolPerformanceSnapshot, history: PerfHistoryPoint[]): string {
    const sys = snap.system_specs;
    const set = snap.active_settings;
    const diag = snap.pipeline_diagnostics;
    const now = new Date(snap.timestamp_ms);

    const lines = [
      '======================================================',
      '        CLIPTOOL · PERFORMANCE & SYSTEM SNAPSHOT      ',
      '======================================================',
      `Zeitpunkt: ${now.toLocaleString()}`,
      `Export-Modus: Vollständiges 60-Sekunden-Schrittprotokoll`,
      `Erfasste Schritte: ${history.length} Abtastungen (Aktuelle Abtastrate: ${perfIntervalMs} ms)`,
      '',
      '--- 💻 SYSTEM & HARDWARE-SPEZIFIKATIONEN ---',
      `Betriebssystem:        ${sys.os_name}`,
      `Prozessor (CPU):       ${sys.cpu_name}`,
      `Logische Kerne:        ${sys.cpu_cores} Kerne`,
      `Gesamter RAM:          ${sys.total_ram_mb.toFixed(0)} MB (Frei/Verfügbar: ${sys.available_ram_mb.toFixed(0)} MB)`,
      `Stromversorgung:       ${sys.power_source}`,
      `Hardware-Tier:         ${sys.hardware_tier}`,
      `Grafikkarte (GPU):     ${sys.gpu_name}`,
      `Monitor-Auflösung:     ${sys.display_resolution} @ ${sys.display_refresh_rate} Hz`,
      `Roh-Pixelstrom:        ${sys.capture_pixel_rate_mps.toFixed(1)} Megapixel / Sekunde`,
      `Unkomprimierte Daten:  ${sys.raw_video_bandwidth_mbs.toFixed(1)} MB / Sekunde (Speicherbus)`,
      `Speicherplatz (Clips): ${sys.disk_free_gb.toFixed(1)} GB frei (Gesamt: ${sys.disk_total_gb.toFixed(0)} GB)`,
      '',
      '--- ⚙️ AKTIVE AUFNAHME-EINSTELLUNGEN ---',
      `Video-Codec:           ${set.video_codec}`,
      `Gewählte Auflösung:    ${set.video_resolution}`,
      `Bildwiederholrate:     ${set.fps} FPS`,
      `Bitrate-Preset:        ${set.bitrate_preset}`,
      `Puffer-Länge:          ${set.buffer_length_secs} Sekunden`,
      `Aufnahme-Monitor:      Display ${set.monitor_idx}`,
      `Mauszeiger im Clip:    ${set.show_cursor ? 'Ja' : 'Nein'}`,
      `HDR Tonemapping:       ${set.hdr_tonemapping ? 'Aktiviert' : 'Deaktiviert'}`,
      `Spike-Erkennung:       ${set.spike_detection ? `Aktiviert (Schwelle: ${set.spike_threshold})` : 'Deaktiviert'}`,
      `Controller-Clipping:   ${set.controller_clipping ? 'Aktiviert' : 'Deaktiviert'}`,
      `Audio-Sync Offset:     ${set.audio_sync_offset_ms} ms`,
      `Isolierte App-Spuren:  ${set.isolated_apps_count} konfiguriert`,
      '',
      '--- 🚀 VIDEO-PIPELINE & HARDWAREBESCHLEUNIGUNG ---',
      `Aktiver Encoder:       ${diag.active_encoder}`,
      `Hardware-Beschleunigung: ${diag.hw_acceleration}`,
      `Ziel-Auflösung:        ${diag.target_resolution}`,
      `Pixel-Entlastung:      ${diag.pixel_reduction_pct > 0 ? '-' + diag.pixel_reduction_pct.toFixed(1) + ' % weniger Pixel als Monitor' : 'Keine (1:1 Nativ)'}`,
      `Skalierungsmethode:    ${diag.scaling_method}`,
      `Video-Puffer Füllstand: ${diag.video_buffer_stored_secs.toFixed(1)} s im RAM (in ${diag.video_buffer_chunks} Chunks)`,
      `Aktive Audio-Spuren:   ${diag.audio_tracks_count} Streams`,
      '',
      '--- 🔍 AUTOMATISCHE FLASCHENHALS-DIAGNOSE ---',
      ...(snap.bottleneck_warnings && snap.bottleneck_warnings.length > 0
        ? snap.bottleneck_warnings.map((w) => `• ${w}`)
        : ['• Keine Flaschenhälse festgestellt']),
      '',
      '--- 📊 TOOL-RESSOURCENVERBRAUCH (EXKLUSIV CLIPTOOL) ---',
      `Gesamt CPU:            ${snap.total_cpu_normalized.toFixed(1)} % (${snap.total_cpu_usage.toFixed(1)} % Core-Last auf ${snap.cpu_cores} Kernen)`,
      `Gesamt RAM (Phys.):    ${snap.total_memory_mb.toFixed(1)} MB (Working Set)`,
      `Gesamt Commit (Virt.): ${snap.total_virtual_memory_mb.toFixed(1)} MB`,
      `Video-Ringpuffer:      ${snap.video_buffer_mb.toFixed(1)} MB / ${snap.video_buffer_max_mb.toFixed(0)} MB (${snap.video_buffer_max_bytes > 0 ? ((snap.video_buffer_bytes / snap.video_buffer_max_bytes) * 100).toFixed(1) : '0.0'} %)`,
      `Audio-Puffer:          ${snap.audio_buffer_mb.toFixed(2)} MB`,
      `Erfasste Prozesse:     ${snap.processes.length}`,
      '',
      '--- ⚡ SUBPROZESS-AUFSCHLÜSSELUNG (PID-BAUM) ---',
      ...snap.processes.map((p) =>
        `[PID ${p.pid}] ${p.name.padEnd(28)} | ${p.role.padEnd(35)} | CPU: ${p.cpu_usage_normalized.toFixed(1)}% (${p.cpu_usage.toFixed(1)}% Core) | RAM: ${p.memory_mb.toFixed(1)}MB | Commit: ${p.virtual_memory_mb.toFixed(1)}MB`
      ),
    ];

    if (history.length > 0) {
      lines.push(
        '',
        '--- ⏱️ 60-SEKUNDEN CHRONOLOGISCHES SCHRITT-PROTOKOLL (JEDER EINZELNE ABTASTSCHRITT) ---',
        'Schritt | Uhrzeit       | Relativ  | Gesamt CPU | Gesamt RAM | Video-Buf | Audio-Buf | FFmpeg (CPU/RAM)   | ClipTool (CPU/RAM) | WebView2 (CPU/RAM) | Status',
        '---------------------------------------------------------------------------------------------------------------------------------------------------------'
      );

      const latestTime = history[history.length - 1].timeMs;
      let cpuMin = 999.0;
      let cpuMax = 0.0;
      let cpuSum = 0.0;
      let maxTimeStr = '';
      let spikeCount = 0;

      history.forEach((pt, idx) => {
        const relSec = ((pt.timeMs - latestTime) / 1000).toFixed(1);
        const relFormatted = relSec.startsWith('-') ? `T${relSec}s` : `T+${relSec}s`;
        const date = new Date(pt.timeMs);
        const timeStr = `${date.toTimeString().split(' ')[0]}.${String(date.getMilliseconds()).padStart(3, '0')}`;

        if (pt.cpuNorm < cpuMin) cpuMin = pt.cpuNorm;
        if (pt.cpuNorm > cpuMax) {
          cpuMax = pt.cpuNorm;
          maxTimeStr = timeStr;
        }
        cpuSum += pt.cpuNorm;

        const isSpike = pt.cpuNorm >= 20.0;
        if (isSpike) spikeCount++;

        let status = 'Normal';
        if (isSpike) {
          if (pt.ffmpegCpu > 12.0) status = '⚠️ SPIKE (FFmpeg)';
          else if (pt.webViewCpu > 10.0) status = '⚠️ SPIKE (WebView2)';
          else if (pt.clipToolCpu > 8.0) status = '⚠️ SPIKE (ClipTool)';
          else status = '⚠️ SPIKE (CPU-Peak)';
        }

        const numStr = String(idx + 1).padStart(3, '0');
        const row = `${numStr}     | ${timeStr} | ${relFormatted.padStart(8)} | ${pt.cpuNorm.toFixed(1).padStart(8)} % | ${pt.ramMb.toFixed(1).padStart(8)} MB | ${pt.vidBufMb.toFixed(1).padStart(7)} MB | ${pt.audBufMb.toFixed(1).padStart(7)} MB | ${(pt.ffmpegCpu.toFixed(1) + '% / ' + pt.ffmpegRam.toFixed(0) + 'MB').padStart(18)} | ${(pt.clipToolCpu.toFixed(1) + '% / ' + pt.clipToolRam.toFixed(0) + 'MB').padStart(18)} | ${(pt.webViewCpu.toFixed(1) + '% / ' + pt.webViewRam.toFixed(0) + 'MB').padStart(18)} | ${status}`;
        lines.push(row);
      });

      const cpuAvg = history.length > 0 ? (cpuSum / history.length) : 0.0;
      const firstRam = history[0].ramMb;
      const lastRam = history[history.length - 1].ramMb;
      const ramDelta = (lastRam - firstRam).toFixed(1);
      const ramDeltaStr = lastRam >= firstRam ? `+${ramDelta} MB` : `${ramDelta} MB`;

      lines.push(
        '---------------------------------------------------------------------------------------------------------------------------------------------------------',
        '',
        '--- 📈 STATISTISCHE 60-SEKUNDEN AUSWERTUNG ---',
        `Erfasste Schritte:      ${history.length} Datenpunkte`,
        `CPU-Minimum:            ${cpuMin.toFixed(1)} %`,
        `CPU-Durchschnitt:       ${cpuAvg.toFixed(1)} %`,
        `CPU-Maximum (Peak):     ${cpuMax.toFixed(1)} % (aufgetreten um ${maxTimeStr})`,
        `Erkannte Spikes (≥20%): ${spikeCount} Mal in den letzten 60 Sekunden`,
        `RAM-Entwicklung:        ${firstRam.toFixed(1)} MB -> ${lastRam.toFixed(1)} MB (${ramDeltaStr})`
      );
    }

    lines.push('======================================================');
    return lines.join('\n');
  }

  $id('btn-perf-copy')?.addEventListener('click', async () => {
    if (!latestSnapshot) {
      toast('Noch keine Daten verfügbar', 'error');
      return;
    }
    const text = buildPerformanceSnapshotText(latestSnapshot, perfHistory);
    try {
      await navigator.clipboard.writeText(text);
      toast('Vollständiger 60s-Diagnose-Snapshot kopiert!', 'success');
    } catch {
      toast('Fehler beim Kopieren in Zwischenablage', 'error');
    }
  });

  $id('btn-perf-export-txt')?.addEventListener('click', async () => {
    if (!latestSnapshot) {
      toast('Noch keine Daten verfügbar', 'error');
      return;
    }
    const text = buildPerformanceSnapshotText(latestSnapshot, perfHistory);
    try {
      const savedPath = await invoke<string>('save_perf_snapshot', { content: text });
      toast(`💾 Snapshot .txt Datei gespeichert!`, 'success');
      console.log('[Snapshot] saved to:', savedPath);
    } catch (err) {
      toast(`Fehler beim Speichern: ${err}`, 'error');
    }
  });

  // Handle window resize for canvas redraw
  const resizeHandler = () => {
    if (currentView === 'performance') {
      const curCanvas = $id<HTMLCanvasElement>('perf-canvas');
      if (curCanvas) drawPerfCanvas(curCanvas);
    }
  };
  window.addEventListener('resize', resizeHandler, { passive: true });
}

// ── Settings View ─────────────────────────────────────────────────────────────
function renderSettings(root: HTMLElement): void {
  root.innerHTML =
    '<div class="view" id="view-settings">' +
      '<div class="view-head">' +
        '<div class="view-title-group">' +
          '<h1>Einstellungen</h1>' +
          '<span class="view-sub">Änderungen werden beim nächsten Aufnahmestart übernommen</span>' +
        '</div>' +
      '</div>' +

      '<section class="card settings-section">' +
        '<div class="card-title">Aufnahme & Video</div>' +
        rowText('Speicherort', 'Ordner, in dem gespeicherte Clips abgelegt werden.',
          '<div class="path-picker" style="display:flex;gap:0.5rem;align-items:center;min-width:0">' +
            '<input type="text" id="set-save-path" readonly value="' + esc(cfg.custom_clip_path) + '" style="flex:1;min-width:0;direction:rtl;text-align:left"/>' +
            '<button class="ghost" id="pick-folder-btn" title="Ordner wählen">' + svgIcon('folder') + '</button>' +
          '</div>'
        ) +
        rowText('Puffer-Länge', 'Rolling RAM-Fenster in Sekunden.',
          '<div style="display:flex;flex-direction:column;gap:0.4rem;align-items:flex-end">' +
            '<div style="display:inline-flex;align-items:center;gap:0.5rem">' +
              '<input type="number" id="set-buffer-len" style="width:90px;text-align:center;font-weight:700" min="5" max="3600" step="5" value="' + cfg.buffer_length_secs + '"/>' +
              '<span style="font-size:0.8rem;font-weight:600;color:var(--text-muted)">Sek.</span>' +
              '<span id="buf-len-hint" style="font-size:0.8rem;font-weight:700;color:var(--accent-2);min-width:3.5rem">' + fmtDur(cfg.buffer_length_secs) + '</span>' +
            '</div>' +
            '<div class="quick-pills" style="display:flex;gap:0.3rem">' +
              [30, 60, 120, 180, 300, 600].map((s) => '<button class="chip set-buf-pill" style="padding:0.15rem 0.5rem;font-size:0.7rem" data-val="' + s + '">' + (s >= 60 ? (s / 60) + 'm' : s + 's') + '</button>').join('') +
            '</div>' +
          '</div>'
        ) +
        rowText('Video Codec', 'Wähle den optimalen Encoder für dein System.',
          '<div class="codec-grid" id="codec-grid">' +
            codecCard('AV1', 'AV1', 'Ultra effizient, minimale Dateigröße') +
            codecCard('HEVC', 'HEVC / H.265', 'Sehr hohe Qualität bei kleinem Speicher') +
            codecCard('H264', 'H.264', 'Universell kompatibel für alle Geräte') +
          '</div>'
        ) +
        rowText('Aufnahme-Monitor', 'Wähle den Bildschirm aus, der aufgenommen werden soll.',
          '<select id="set-monitor" style="min-width:220px"><option value="0">Lade Monitore…</option></select>'
        ) +
        rowText('Auflösung', 'Ziel-Auflösung der Video-Aufnahme (Pure VRAM GPU-Skalierung).',
          '<select id="set-resolution" style="min-width:220px">' +
            [
              ['Original', 'Original (Nativ - Pure VRAM Zero-Copy)'],
              ['1440p', '2560 × 1440 (1440p / 2K)'],
              ['1080p', '1920 × 1080 (1080p / Full HD)'],
              ['900p', '1600 × 900 (900p / HD+)'],
              ['720p', '1280 × 720 (720p / HD - GPU-Skaliert)'],
              ['540p', '960 × 540 (540p / qHD)'],
              ['480p', '854 × 480 (480p / SD)'],
              ['360p', '640 × 360 (360p / Low)'],
              ['240p', '426 × 240 (240p / Ultra Low)'],
            ].map(([v, l]) => option(v, l, cfg.video_resolution)).join('') +
          '</select>'
        ) +
        rowText('Framerate (FPS)', 'Bildrate der Video-Aufzeichnung.',
          '<div style="display:flex;flex-direction:column;gap:0.4rem;align-items:flex-end">' +
            '<div style="display:inline-flex;align-items:center;gap:0.5rem">' +
              '<input type="number" id="set-fps" style="width:85px;text-align:center;font-weight:700" min="10" max="360" step="1" value="' + esc(cfg.fps_selection) + '"/>' +
              '<span style="font-size:0.8rem;font-weight:600;color:var(--text-muted)">FPS</span>' +
            '</div>' +
            '<div class="quick-pills" style="display:flex;gap:0.3rem">' +
              [30, 45, 60, 120, 144, 240].map((f) => '<button class="chip set-fps-pill" style="padding:0.15rem 0.5rem;font-size:0.7rem" data-val="' + f + '">' + f + '</button>').join('') +
            '</div>' +
          '</div>'
        ) +
        rowText('Video-Bitrate', 'Qualitätsstufe für die Bitrate.',
          '<div class="seg-control" id="bitrate-seg">' +
            ['Low', 'Balanced', 'High', 'Ultra'].map((p) =>
              '<button class="seg-btn' + (cfg.bitrate_preset === p ? ' seg-active' : '') + '" data-preset="' + p + '">' + p + '</button>').join('') +
          '</div>'
        ) +
        rowText('HDR-zu-SDR Tonemapping', 'Verhindert ausgewaschene oder blasse Farben bei Aufnahmen auf HDR-Monitoren.',
          '<label class="switch"><input type="checkbox" id="set-hdr-tonemap"' + (cfg.hdr_tonemapping ? ' checked' : '') + '/><span class="track"></span></label>'
        ) +
        rowText('Mauszeiger aufnehmen', 'Mauszeiger im Video aufzeichnen.',
          '<label class="switch"><input type="checkbox" id="set-show-cursor"' + (cfg.show_cursor_in_clips !== false ? ' checked' : '') + '/><span class="track"></span></label>'
        ) +
        rowText('Auto-Clipboard (Strg+V)', 'Automatisch neu gespeicherte Clips in die Windows-Zwischenablage kopieren.',
          '<label class="switch"><input type="checkbox" id="set-auto-clipboard"' + (cfg.auto_clipboard ? ' checked' : '') + '/><span class="track"></span></label>'
        ) +
      '</section>' +

      '<section class="card settings-section">' +
        '<div class="card-title">Multi-Dauer Hotkeys & Steuerung</div>' +
        '<div class="hotkey-list" id="hotkey-slot-list"></div>' +
        '<div style="margin-top:0.6rem">' +
          '<button class="ghost" id="btn-add-hotkey" style="font-size:0.78rem">+ Weiteren Hotkey hinzufügen</button>' +
        '</div>' +
        rowText('In-Game Overlay Benachrichtigung', 'Dezenter Toast über Vollbild-Spielen ohne jeden Fokus-Diebstahl.',
          '<label class="switch"><input type="checkbox" id="set-overlay-notify"' + (cfg.overlay_notification !== false ? ' checked' : '') + '/><span class="track"></span></label>'
        ) +
        rowText('Puffer bei Inaktivität pausieren', 'Pausiert den Puffer automatisch bei PC-Leerlauf und spart Strom.',
          '<div style="display:flex;align-items:center;gap:0.75rem">' +
            '<label class="switch"><input type="checkbox" id="set-auto-pause-idle"' + (cfg.auto_pause_idle ? ' checked' : '') + '/><span class="track"></span></label>' +
            '<select id="set-idle-mins" style="min-width:115px"' + (!cfg.auto_pause_idle ? ' disabled' : '') + '>' +
              [
                ['1', 'nach 1 Min'],
                ['3', 'nach 3 Min'],
                ['5', 'nach 5 Min'],
                ['10', 'nach 10 Min'],
                ['15', 'nach 15 Min'],
                ['30', 'nach 30 Min'],
              ].map(([v, l]) => option(v, l, String(cfg.auto_pause_idle_minutes || 5))).join('') +
            '</select>' +
          '</div>'
        ) +
        rowText('Controller-Clipping (Gamepad)', 'Speichern per Xbox/Gamepad-Kombination (LB + RB + D-Pad Down oder Back + Start).',
          '<label class="switch"><input type="checkbox" id="set-controller-clip"' + (cfg.controller_clipping ? ' checked' : '') + '/><span class="track"></span></label>'
        ) +
        rowText('Beim Windows-Start ausführen', 'ClipTool automatisch im Hintergrund starten.',
          '<label class="switch"><input type="checkbox" id="set-autostart"' + (cfg.autostart ? ' checked' : '') + '/><span class="track"></span></label>'
        ) +
      '</section>' +

      '<section class="card settings-section">' +
        '<div class="card-title">Speicherplatz & Auto-Cleanup</div>' +
        '<div class="cleanup-grid">' +
          '<div class="cleanup-card">' +
            '<span class="cleanup-card-label">Max. Clip-Alter</span>' +
            '<select id="cleanup-age">' +
              [
                ['0', 'Nie nach Alter löschen'],
                ['7', 'Älter als 7 Tage'],
                ['14', 'Älter als 14 Tage'],
                ['30', 'Älter als 30 Tage'],
                ['60', 'Älter als 60 Tage'],
                ['90', 'Älter als 90 Tage'],
              ].map(([v, l]) => option(v, l, String(cfg.cleanup?.max_age_days ?? 30))).join('') +
            '</select>' +
          '</div>' +
          '<div class="cleanup-card">' +
            '<span class="cleanup-card-label">Max. Clip-Ordnergröße</span>' +
            '<select id="cleanup-storage">' +
              [
                ['0', 'Kein Limit'],
                ['10', 'Max. 10 GB'],
                ['25', 'Max. 25 GB'],
                ['50', 'Max. 50 GB'],
                ['100', 'Max. 100 GB'],
                ['200', 'Max. 200 GB'],
              ].map(([v, l]) => option(v, l, String(Math.round(cfg.cleanup?.max_storage_gb ?? 50)))).join('') +
            '</select>' +
          '</div>' +
          '<div class="cleanup-card">' +
            '<span class="cleanup-card-label">Min. freier Festplattenspeicher</span>' +
            '<select id="cleanup-disk">' +
              [
                ['0', 'Deaktiviert'],
                ['10', 'Unter 10 GB frei'],
                ['15', 'Unter 15 GB frei'],
                ['25', 'Unter 25 GB frei'],
                ['50', 'Unter 50 GB frei'],
              ].map(([v, l]) => option(v, l, String(Math.round(cfg.cleanup?.min_free_disk_gb ?? 15)))).join('') +
            '</select>' +
          '</div>' +
        '</div>' +
        '<div style="display:flex;justify-content:space-between;align-items:center;margin-top:0.8rem;flex-wrap:wrap;gap:0.75rem">' +
          '<span class="cleanup-banner-note">⭐ Favorisierte Clips werden niemals automatisch gelöscht.</span>' +
          '<button class="ghost" id="btn-manual-cleanup">🧹 Jetzt bereinigen</button>' +
        '</div>' +
      '</section>' +

      '<section class="card settings-section">' +
        '<div class="card-title">Audio & Highlights</div>' +
        rowText('Automatische Multi-Spuren', 'Jedes sichtbare Programm (Discord, Spotify, Spiel) erhält automatisch eine eigene Audiospur.',
          '<div class="chips-row" id="settings-split-chips" style="max-width:320px"></div>'
        ) +
        rowText('Mikrofon-Verstärkung', '50% = normaler Standard-Pegel. Bis zu 100% (+6dB Boost).',
          (() => {
            const pct = Math.round((cfg.mic_volume !== undefined ? cfg.mic_volume : 1.0) * 50);
            return '<div style="display:flex;align-items:center;gap:0.75rem;min-width:200px">' +
              '<input type="range" id="set-mic-volume" min="0" max="100" step="1" value="' + pct + '" style="--fill:' + pct + '%;flex:1"/>' +
              '<span id="set-mic-volume-val" style="min-width:3rem;font-weight:750;font-size:0.82rem;text-align:right;font-variant-numeric:tabular-nums">' + pct + '%</span>' +
            '</div>';
          })()
        ) +
        rowText('Audio-Sync Offset', 'Feinjustierung der Bild- und Ton-Synchronisation (z.B. für Bluetooth-Kopfhörer).',
          (() => {
            const offset = cfg.audio_sync_offset_ms || 0;
            return '<div style="display:flex;align-items:center;gap:0.75rem;min-width:200px">' +
              '<input type="range" id="set-audio-sync" min="-1000" max="1000" step="25" value="' + offset + '" style="--fill:' + (((offset + 1000) / 2000 * 100)).toFixed(0) + '%;flex:1"/>' +
              '<span id="set-audio-sync-val" style="min-width:3.5rem;font-weight:750;font-size:0.82rem;text-align:right;font-variant-numeric:tabular-nums">' + (offset > 0 ? '+' : '') + offset + ' ms</span>' +
            '</div>';
          })()
        ) +
        rowText('Highlight / Hype-Erkennung', 'Automatische Erkennung von Schrei- und Hype-Momenten auf dem Mikrofon zur Timeline-Markierung.',
          '<label class="switch"><input type="checkbox" id="set-spike-enabled"' + (cfg.spike_detection_enabled !== false ? ' checked' : '') + '/><span class="track"></span></label>'
        ) +
        rowText('Hype-Empfindlichkeit', 'Schwellenwert für die Pegelspitzen-Erkennung.',
          (() => {
            const dbVal = Math.round((cfg.spike_threshold || 0.318) * 22 + 3);
            return '<input type="range" id="set-spike-threshold" min="3" max="25" step="1" value="' + dbVal + '" style="--fill:' + (((dbVal - 3) / 22 * 100)).toFixed(0) + '%"/>';
          })()
        ) +
      '</section>' +
    '</div>';

  // Save location picker
  $id('pick-folder-btn')?.addEventListener('click', async () => {
    const picked = await open({ directory: true, multiple: false }) as string | null;
    if (picked) {
      cfg.custom_clip_path = picked;
      ($id<HTMLInputElement>('set-save-path')).value = picked;
      await saveConfig();
      toast('Speicherort aktualisiert', 'success');
    }
  });

  // Codec cards selection
  const currentCodec = cfg.video_codec || 'H264';
  root.querySelectorAll<HTMLElement>('.codec-card').forEach((card) => {
    if (card.dataset.codec === currentCodec) card.classList.add('selected');
    card.addEventListener('click', async () => {
      root.querySelectorAll('.codec-card').forEach((c) => c.classList.remove('selected'));
      card.classList.add('selected');
      cfg.video_codec = card.dataset.codec as 'H264' | 'HEVC' | 'AV1';
      await saveConfig();
      await restartIfRunning();
      toast('Codec: ' + cfg.video_codec, 'success');
    });
  });

  // Monitor select
  void invoke<MonitorInfo[]>('get_available_monitors').then((mons) => {
    const sel = $id<HTMLSelectElement>('set-monitor');
    if (sel && mons.length > 0) {
      sel.innerHTML = mons.map(m => '<option value="' + m.index + '"' + ((cfg.monitor_idx ?? 0) === m.index ? ' selected' : '') + '>' + esc(m.name) + '</option>').join('');
    }
  }).catch(() => {});

  $id<HTMLSelectElement>('set-monitor')?.addEventListener('change', async (e) => {
    cfg.monitor_idx = parseInt((e.target as HTMLSelectElement).value, 10) || 0;
    await saveConfig();
    await restartIfRunning();
    toast('Aufnahme-Monitor aktualisiert', 'success');
  });

  // Controller clipping toggle
  $id<HTMLInputElement>('set-controller-clip')?.addEventListener('change', async (e) => {
    cfg.controller_clipping = (e.target as HTMLInputElement).checked;
    await saveConfig();
    toast(cfg.controller_clipping ? '🎮 Controller-Clipping aktiviert' : '🎮 Controller-Clipping deaktiviert', 'info');
  });

  // HDR Tonemapping toggle
  $id<HTMLInputElement>('set-hdr-tonemap')?.addEventListener('change', async (e) => {
    cfg.hdr_tonemapping = (e.target as HTMLInputElement).checked;
    await saveConfig();
    await restartIfRunning();
    toast(cfg.hdr_tonemapping ? 'HDR-Tonemapping aktiviert' : 'HDR-Tonemapping deaktiviert', 'info');
  });

  // Show cursor toggle
  $id<HTMLInputElement>('set-show-cursor')?.addEventListener('change', async (e) => {
    cfg.show_cursor_in_clips = (e.target as HTMLInputElement).checked;
    await saveConfig();
    await restartIfRunning();
    toast(cfg.show_cursor_in_clips ? 'Mauszeiger wird aufgenommen' : 'Mauszeiger ausgeblendet', 'info');
  });

  // Autostart toggle
  $id<HTMLInputElement>('set-autostart')?.addEventListener('change', async (e) => {
    const isChecked = (e.target as HTMLInputElement).checked;
    cfg.autostart = isChecked;
    await saveConfig();
    try {
      if (isChecked) {
        await invoke('enable_autostart');
      } else {
        await invoke('disable_autostart');
      }
      toast(isChecked ? 'Autostart aktiviert' : 'Autostart deaktiviert', 'info');
    } catch (err) {
      toast(`Autostart-Fehler: ${err}`, 'error');
    }
  });

  // In-Game Overlay toggle
  $id<HTMLInputElement>('set-overlay-notify')?.addEventListener('change', async (e) => {
    cfg.overlay_notification = (e.target as HTMLInputElement).checked;
    await saveConfig();
    toast(cfg.overlay_notification ? 'Overlay-Benachrichtigungen aktiv' : 'Overlay deaktiviert', 'info');
  });

  // Auto-pause idle toggle & minute select
  const idleToggle = $id<HTMLInputElement>('set-auto-pause-idle');
  const idleMinsSelect = $id<HTMLSelectElement>('set-idle-mins');
  idleToggle?.addEventListener('change', async (e) => {
    cfg.auto_pause_idle = (e.target as HTMLInputElement).checked;
    if (idleMinsSelect) idleMinsSelect.disabled = !cfg.auto_pause_idle;
    await saveConfig();
    toast(cfg.auto_pause_idle ? 'Auto-Pause bei Leerlauf aktiv' : 'Auto-Pause deaktiviert', 'info');
  });
  idleMinsSelect?.addEventListener('change', async (e) => {
    cfg.auto_pause_idle_minutes = parseInt((e.target as HTMLSelectElement).value, 10) || 5;
    await saveConfig();
  });

  // Auto-Cleanup settings
  if (!cfg.cleanup) {
    cfg.cleanup = { enabled: true, max_age_days: 30, max_storage_gb: 50.0, min_free_disk_gb: 15.0 };
  }
  $id<HTMLSelectElement>('cleanup-age')?.addEventListener('change', async (e) => {
    if (cfg.cleanup) cfg.cleanup.max_age_days = parseInt((e.target as HTMLSelectElement).value, 10);
    await saveConfig();
    toast('Cleanup-Regel aktualisiert', 'success');
  });
  $id<HTMLSelectElement>('cleanup-storage')?.addEventListener('change', async (e) => {
    if (cfg.cleanup) cfg.cleanup.max_storage_gb = parseFloat((e.target as HTMLSelectElement).value);
    await saveConfig();
    toast('Cleanup-Regel aktualisiert', 'success');
  });
  $id<HTMLSelectElement>('cleanup-disk')?.addEventListener('change', async (e) => {
    if (cfg.cleanup) cfg.cleanup.min_free_disk_gb = parseFloat((e.target as HTMLSelectElement).value);
    await saveConfig();
    toast('Cleanup-Regel aktualisiert', 'success');
  });

  // Manual cleanup trigger button
  $id('btn-manual-cleanup')?.addEventListener('click', async () => {
    const btn = $id<HTMLButtonElement>('btn-manual-cleanup');
    if (btn) btn.disabled = true;
    try {
      const summary = await invoke<{ clips_deleted: number; space_freed_mb: number }>('trigger_auto_cleanup');
      if (summary.clips_deleted > 0) {
        toast(`Bereinigung fertig: ${summary.clips_deleted} alte Clips gelöscht (${summary.space_freed_mb} MB freigegeben)`, 'success');
        libraryLoaded = false;
        await loadLibrary();
      } else {
        toast('Keine zu bereinigenden alten Clips gefunden', 'info');
      }
    } catch (err) {
      toast('Fehler bei Bereinigung: ' + String(err), 'error');
    } finally {
      if (btn) btn.disabled = false;
    }
  });

  // Multi-Hotkey Slots Manager
  function renderHotkeySlotsUI(): void {
    const container = $id('hotkey-slot-list');
    if (!container) return;
    if (!cfg.hotkeys || cfg.hotkeys.length === 0) {
      cfg.hotkeys = [
        { id: 'slot_1', hotkey: 'Alt+C', duration_secs: 30, label: 'Kurzer Clip' },
        { id: 'slot_2', hotkey: 'Alt+Shift+C', duration_secs: 120, label: 'Standard Clip' },
        { id: 'slot_3', hotkey: 'Alt+Ctrl+C', duration_secs: 0, label: 'Ganzer Puffer' },
      ];
    }

    container.innerHTML = cfg.hotkeys.map((slot, idx) => {
      const durHint = slot.duration_secs === 0
        ? 'Puffer'
        : (slot.duration_secs >= 60 ? (slot.duration_secs / 60).toFixed(1) + 'm' : slot.duration_secs + 's');
      return '<div class="hotkey-slot-row" data-idx="' + idx + '">' +
        '<input type="text" class="hotkey-slot-label" value="' + esc(slot.label) + '" placeholder="Bezeichnung…" title="Bezeichnung bearbeiten"/>' +
        '<div style="display:flex;align-items:center;gap:0.45rem">' +
          '<input type="number" class="hotkey-slot-dur-inp" min="0" max="3600" step="1" value="' + slot.duration_secs + '" placeholder="Sek." style="width:72px;text-align:center;font-weight:750" title="Cliplänge in Sekunden (0 = ganzer Puffer)"/>' +
          '<span class="hotkey-slot-dur-hint" style="font-size:0.75rem;font-weight:700;color:var(--accent-2);min-width:3.2rem">' + durHint + '</span>' +
        '</div>' +
        '<button class="hotkey-btn slot-key-btn">' + esc(slot.hotkey || 'Tasten drücken…') + '</button>' +
        (cfg.hotkeys!.length > 1 ? '<button class="btn-del-slot" title="Hotkey löschen">' + svgIcon('trash') + '</button>' : '<div></div>') +
      '</div>';
    }).join('');

    container.querySelectorAll<HTMLInputElement>('.hotkey-slot-label').forEach((inp, idx) => {
      inp.addEventListener('change', async () => {
        if (cfg.hotkeys && cfg.hotkeys[idx]) {
          cfg.hotkeys[idx].label = inp.value.trim() || 'Clip';
          await saveConfig();
        }
      });
    });

    container.querySelectorAll<HTMLDivElement>('.hotkey-slot-row').forEach((row, idx) => {
      const durInp = row.querySelector<HTMLInputElement>('.hotkey-slot-dur-inp');
      const durHint = row.querySelector<HTMLElement>('.hotkey-slot-dur-hint');

      durInp?.addEventListener('input', () => {
        const val = parseInt(durInp.value, 10);
        if (isNaN(val) || val <= 0) {
          if (durHint) durHint.textContent = 'Puffer';
        } else if (val >= 60) {
          if (durHint) durHint.textContent = (val / 60).toFixed(1) + 'm';
        } else {
          if (durHint) durHint.textContent = val + 's';
        }
      });

      durInp?.addEventListener('change', async () => {
        let val = parseInt(durInp.value, 10);
        if (isNaN(val) || val < 0) val = 0;
        if (val > 3600) val = 3600;
        durInp.value = String(val);

        if (cfg.hotkeys && cfg.hotkeys[idx]) {
          cfg.hotkeys[idx].duration_secs = val;
          await saveConfig();
          await invoke('set_hotkey_bindings', { bindings: cfg.hotkeys });
          toast(`Cliplänge für ${cfg.hotkeys[idx].label}: ${val === 0 ? 'Ganzer Puffer' : val + ' Sek.'}`, 'success');
        }
      });
    });

    container.querySelectorAll<HTMLButtonElement>('.btn-del-slot').forEach((btn, idx) => {
      btn.addEventListener('click', async () => {
        if (cfg.hotkeys && cfg.hotkeys.length > 1) {
          cfg.hotkeys.splice(idx, 1);
          await saveConfig();
          await invoke('set_hotkey_bindings', { bindings: cfg.hotkeys });
          renderHotkeySlotsUI();
          toast('Hotkey entfernt', 'info');
        }
      });
    });

    container.querySelectorAll<HTMLButtonElement>('.slot-key-btn').forEach((btn, idx) => {
      btn.addEventListener('click', () => {
        btn.textContent = 'Tasten drücken…';
        btn.style.borderColor = 'var(--accent)';
        const onKey = async (ev: KeyboardEvent) => {
          ev.preventDefault();
          ev.stopPropagation();
          if (ev.key === 'Escape') {
            btn.textContent = cfg.hotkeys?.[idx]?.hotkey || 'Tasten drücken…';
            btn.style.borderColor = '';
            window.removeEventListener('keydown', onKey, true);
            return;
          }
          if (['Control', 'Shift', 'Alt', 'Meta'].includes(ev.key)) return;
          window.removeEventListener('keydown', onKey, true);
          btn.style.borderColor = '';

          const parts: string[] = [];
          if (ev.ctrlKey) parts.push('Ctrl');
          if (ev.shiftKey) parts.push('Shift');
          if (ev.altKey) parts.push('Alt');
          if (ev.metaKey) parts.push('Super');

          let keyName = '';
          if (ev.code.startsWith('Key')) {
            keyName = ev.code.replace('Key', '').toUpperCase();
          } else if (ev.code.startsWith('Digit')) {
            keyName = ev.code.replace('Digit', '');
          } else if (ev.key === ' ' || ev.code === 'Space') {
            keyName = 'Space';
          } else if (/^F\d{1,2}$/i.test(ev.key)) {
            keyName = ev.key.toUpperCase();
          } else if (ev.code.startsWith('Numpad')) {
            const num = ev.code.replace('Numpad', '');
            keyName = /^\d$/.test(num) ? `Num${num}` : num;
          } else if (ev.key === 'ArrowUp') {
            keyName = 'Up';
          } else if (ev.key === 'ArrowDown') {
            keyName = 'Down';
          } else if (ev.key === 'ArrowLeft') {
            keyName = 'Left';
          } else if (ev.key === 'ArrowRight') {
            keyName = 'Right';
          } else if (['Insert', 'Delete', 'Home', 'End', 'PageUp', 'PageDown', 'PrintScreen', 'ScrollLock', 'Pause'].includes(ev.key)) {
            keyName = ev.key;
          } else if (ev.key.length === 1) {
            keyName = ev.key.toUpperCase();
          } else {
            keyName = ev.key;
          }

          const isStandaloneAllowed = /^F\d{1,2}$/i.test(keyName) || ['PrintScreen', 'ScrollLock', 'Pause', 'Insert'].includes(keyName);
          if (parts.length === 0 && !isStandaloneAllowed) {
            btn.textContent = cfg.hotkeys?.[idx]?.hotkey || 'Tasten drücken…';
            toast('Verwende für Buchstaben mindestens eine Taste wie Ctrl, Alt oder Shift (z.B. Alt+C). F-Tasten wie F9 oder F10 funktionieren auch einzeln.', 'error');
            return;
          }

          parts.push(keyName);
          const combo = parts.join('+');
          if (cfg.hotkeys && cfg.hotkeys[idx]) {
            // If another slot already had this exact hotkey, clear it from that slot
            let reassignedFrom = '';
            cfg.hotkeys.forEach((slot, otherIdx) => {
              if (otherIdx !== idx && slot.hotkey && slot.hotkey.toLowerCase() === combo.toLowerCase()) {
                reassignedFrom = slot.label || `Slot ${otherIdx + 1}`;
                slot.hotkey = '';
              }
            });

            cfg.hotkeys[idx].hotkey = combo;
            await saveConfig();
            try {
              await invoke('set_hotkey_bindings', { bindings: cfg.hotkeys });
              renderHotkeySlotsUI();
              if (reassignedFrom) {
                toast(`Hotkey ${combo} gesetzt (wurde von '${reassignedFrom}' übernommen)`, 'info');
              } else {
                toast('Hotkey gesetzt: ' + combo, 'success');
              }
            } catch (err) {
              renderHotkeySlotsUI();
              toast('Fehler beim Registrieren: ' + String(err), 'error');
            }
          }
        };
        window.addEventListener('keydown', onKey, true);
      });
    });
  }

  renderHotkeySlotsUI();

  $id('btn-add-hotkey')?.addEventListener('click', async () => {
    if (!cfg.hotkeys) cfg.hotkeys = [];
    const newId = 'slot_' + (Date.now() % 100000);
    cfg.hotkeys.push({
      id: newId,
      hotkey: 'Alt+Shift+K',
      duration_secs: 60,
      label: 'Clip ' + (cfg.hotkeys.length + 1)
    });
    await saveConfig();
    await invoke('set_hotkey_bindings', { bindings: cfg.hotkeys });
    renderHotkeySlotsUI();
    toast('Neuer Hotkey hinzugefügt', 'success');
  });

  // Resolution select
  const resSelect = $id<HTMLSelectElement>('set-resolution');
  resSelect?.addEventListener('change', async () => {
    cfg.video_resolution = resSelect.value;
    await saveConfig();
    await restartIfRunning();
  });

  // Buffer length slider & pills
  const bufInput = $id<HTMLInputElement>('set-buffer-len');
  const bufHint = $id<HTMLElement>('buf-len-hint');
  const updateBufHint = (s: number) => { if (bufHint) bufHint.textContent = fmtDur(s); };
  bufInput?.addEventListener('input', () => {
    const val = parseInt(bufInput.value, 10);
    if (!isNaN(val) && val > 0) updateBufHint(val);
  });
  bufInput?.addEventListener('change', async () => {
    let val = parseInt(bufInput.value, 10);
    if (isNaN(val) || val < 5) val = 5;
    if (val > 3600) val = 3600;
    bufInput.value = String(val);
    updateBufHint(val);
    cfg.buffer_length_secs = val;
    await saveConfig();
    await restartIfRunning();
  });
  root.querySelectorAll<HTMLButtonElement>('.set-buf-pill').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const val = parseInt(btn.dataset.val!, 10);
      if (bufInput) bufInput.value = String(val);
      updateBufHint(val);
      cfg.buffer_length_secs = val;
      await saveConfig();
      await restartIfRunning();
    });
  });

  // FPS number input & pills
  const fpsInput = $id<HTMLInputElement>('set-fps');
  fpsInput?.addEventListener('change', async () => {
    let val = parseInt(fpsInput.value, 10);
    if (isNaN(val) || val < 10) val = 10;
    if (val > 360) val = 360;
    fpsInput.value = String(val);
    cfg.fps_selection = String(val);
    await saveConfig();
    await restartIfRunning();
  });
  root.querySelectorAll<HTMLButtonElement>('.set-fps-pill').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const val = btn.dataset.val!;
      if (fpsInput) fpsInput.value = val;
      cfg.fps_selection = val;
      await saveConfig();
      await restartIfRunning();
    });
  });

  // Bitrate segmented control
  $id('bitrate-seg')?.querySelectorAll<HTMLButtonElement>('.seg-btn').forEach((btn) =>
    btn.addEventListener('click', async () => {
      $id('bitrate-seg').querySelectorAll('.seg-btn').forEach((b) => b.classList.remove('seg-active'));
      btn.classList.add('seg-active');
      cfg.bitrate_preset = btn.dataset.preset!;
      await saveConfig();
      await restartIfRunning();
    }));

  // Auto clipboard switch
  $id<HTMLInputElement>('set-auto-clipboard')?.addEventListener('change', async (ev) => {
    cfg.auto_clipboard = (ev.target as HTMLInputElement).checked;
    await saveConfig();
  });

  // Microphone volume slider
  const micVolSlider = $id<HTMLInputElement>('set-mic-volume');
  const micVolVal = $id<HTMLElement>('set-mic-volume-val');
  const paintMicVolFill = () => {
    if (!micVolSlider) return;
    const val = parseInt(micVolSlider.value, 10);
    micVolSlider.style.setProperty('--fill', val + '%');
    if (micVolVal) micVolVal.textContent = val + '%';
    const gain = val / 50.0;
    void invoke('set_mic_volume', { volume: gain });
  };
  micVolSlider?.addEventListener('input', paintMicVolFill);
  micVolSlider?.addEventListener('change', async () => {
    const val = parseInt(micVolSlider.value, 10);
    cfg.mic_volume = val / 50.0;
    await saveConfig();
  });

  // Audio Sync Offset slider
  const audioSyncSlider = $id<HTMLInputElement>('set-audio-sync');
  const audioSyncVal = $id<HTMLElement>('set-audio-sync-val');
  const paintAudioSyncFill = () => {
    if (!audioSyncSlider) return;
    const val = parseInt(audioSyncSlider.value, 10);
    audioSyncSlider.style.setProperty('--fill', (((val + 1000) / 2000) * 100).toFixed(0) + '%');
    if (audioSyncVal) audioSyncVal.textContent = (val > 0 ? '+' : '') + val + ' ms';
  };
  audioSyncSlider?.addEventListener('input', paintAudioSyncFill);
  audioSyncSlider?.addEventListener('change', async () => {
    const val = parseInt(audioSyncSlider.value, 10);
    cfg.audio_sync_offset_ms = val;
    await saveConfig();
    toast('Audio-Sync Offset: ' + (val > 0 ? '+' : '') + val + ' ms', 'info');
  });

  // Spike detection toggle & threshold slider
  $id<HTMLInputElement>('set-spike-enabled')?.addEventListener('change', async (ev) => {
    cfg.spike_detection_enabled = (ev.target as HTMLInputElement).checked;
    await saveConfig();
    await restartIfRunning();
  });

  const spikeSlider = $id<HTMLInputElement>('set-spike-threshold');
  const paintSpikeFill = () => {
    if (!spikeSlider) return;
    const val = parseFloat(spikeSlider.value);
    spikeSlider.style.setProperty('--fill', (((val - 3) / 22) * 100).toFixed(0) + '%');
  };
  spikeSlider?.addEventListener('input', paintSpikeFill);
  spikeSlider?.addEventListener('change', async () => {
    const dbVal = parseInt(spikeSlider.value, 10);
    cfg.spike_threshold = parseFloat(((dbVal - 3) / 22).toFixed(3));
    await saveConfig();
    await restartIfRunning();
  });

  refreshSplitChips('settings-split-chips');
}

function rowText(title: string, desc: string, controlHtml: string): string {
  return '<div class="setting-row"><div class="setting-label">' +
    '<span class="setting-title">' + title + '</span>' +
    (desc ? '<span class="setting-desc">' + desc + '</span>' : '') +
    '</div>' + controlHtml + '</div>';
}

function codecCard(key: string, title: string, desc: string): string {
  return '<div class="codec-card" data-codec="' + key + '">' +
    '<div class="codec-title">' + title + '</div>' +
    '<div class="codec-sub">' + desc + '</div>' +
  '</div>';
}

function option(value: string, label: string, selected: string): string {
  return '<option value="' + value + '"' + (value === selected ? ' selected' : '') + '>' + label + '</option>';
}

async function saveConfig(): Promise<void> {
  try {
    if (cfg.hotkeys && cfg.hotkeys.length > 0) {
      cfg.hotkey = cfg.hotkeys[0].hotkey;
    }
    await invoke('save_config', { config: cfg });
  } catch (e) { toast(String(e), 'error'); }
}

// ── Window Controls ───────────────────────────────────────────────────────────
function setupWindowControls(): void {
  appWindow = getCurrentWindow();

  const titlebar = document.querySelector('.custom-titlebar');
  titlebar?.addEventListener('mousedown', async (e) => {
    const mouseEv = e as MouseEvent;
    // Only drag with primary left mouse button and not on interactive buttons
    if (mouseEv.button === 0 && !(mouseEv.target as HTMLElement).closest('button, a, input, select, .titlebar-controls')) {
      if (mouseEv.detail === 2) {
        try { await appWindow?.toggleMaximize(); } catch { await invoke('app_toggle_maximize'); }
      } else {
        try { await appWindow?.startDragging(); } catch {}
      }
    }
  });

  $id('top-status-indicator')?.addEventListener('click', (e) => {
    if (bufferState === 'error') {
      e.stopPropagation();
      void copyDiagnosticReport();
    }
  });

  $id('titlebar-minimize')?.addEventListener('click', async (e) => {
    e.preventDefault();
    e.stopPropagation();
    try { await appWindow?.minimize(); } catch { await invoke('app_minimize'); }
  });
  $id('titlebar-maximize')?.addEventListener('click', async (e) => {
    e.preventDefault();
    e.stopPropagation();
    try { await appWindow?.toggleMaximize(); } catch { await invoke('app_toggle_maximize'); }
  });
  $id('titlebar-close')?.addEventListener('click', async (e) => {
    e.preventDefault();
    e.stopPropagation();
    try { await appWindow?.close(); } catch { await invoke('app_close'); }
  });
}

// ── Initialization ────────────────────────────────────────────────────────────
async function init(): Promise<void> {
  cfg = await invoke<AppConfig>('get_config');
  await invoke('set_mic_volume', { volume: cfg.mic_volume !== undefined ? cfg.mic_volume : 1.0 }).catch(() => {});

  initSidebar();
  setupWindowControls();

  await Promise.all([
    listen<{ state: string; detail: string | null }>('buffer://state', (ev) => {
      updateBufferState(ev.payload.state as BufferState, ev.payload.detail ?? '');
    }),
    listen<{ path: string; game: string }>('clips://saved', async (ev) => {
      libraryLoaded = false;
      const name = ev.payload.path.replace(/^.*[/\\\\]/, '');
      toast('Clip gespeichert: ' + name, 'success');
      await loadLibrary();
      if (document.querySelector('#view-home')) loadRecents();
      if (document.querySelector('#view-library')) paintLibrary();
    }),
  ]);

  const st = await invoke<string>('get_buffer_state');
  updateBufferState(st as BufferState);

  // Replay-buffer automatically starts rolling buffer
  void startBuffer();

  startBufferTicker();
  showView('home');
}

void init();


