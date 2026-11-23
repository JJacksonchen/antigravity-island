(() => {
  const CLOCK_SVG = `<svg viewBox="0 0 16 16" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="8" cy="8" r="6.2"/><path d="M8 4.6V8l2.3 1.4"/></svg>`;
  const CHECK_SVG = `<svg viewBox="0 0 16 16" width="10" height="10" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3.2 8.4l3.1 3.1 6.5-6.8"/></svg>`;

  const I18N = {
    en: {
      waiting: "Waiting for Antigravity",
      waitingDesc: "Open Antigravity App / IDE or configure agy statusLine to sync real-time quota.",
      retry: "Retry Sync",
      demoPreview: "Preview Demo",
      skinStudio: "Skin Studio",
      openFolder: "Open Folder",
      gauge: "Gauge",
      accents: "Accents",
      radius: "Radius",
      opacity: "Opacity",
      glow: "Glow",
      saveToml: "Save TOML",
      customSkinPh: "My Custom Skin",
      prefs: "Preferences",
      display: "Display",
      pillStyle: "Pill Style",
      position: "Position",
      resetPos: "Top Center",
      menubarText: "Menu Bar Text",
      syncRate: "Sync Rate",
      language: "Language",
      alerts: "Low Alerts",
      demoMode: "Demo Mode",
      hide: "Hide",
      hideTip: "Hide Island (restore from tray)",
      collapse: "Collapse",
      dragHeaderTip: "Drag to move · Double-click to reset top center",
      pillTip: "Hover/click to expand · Drag to move · Right-click to hide",
      justNow: "just now",
      secAgo: (s) => `${s}s ago`,
      minAgo: (m) => `${m}m ago`,
      resetIn: (t) => `Resets in ${t}`,
      resetDue: "Resetting…",
      resetAt: (tm) => `at ${tm}`,
      wkTag: "7d",
      fiveHourName: "5-Hour Quota",
      weeklyName: "Weekly Quota",
      sessionName: "Session Quota",
      fullTag: "Full",
      fullQuota: "Full quota",
      weekdays: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
      offlinePill: "AGY Offline",
      syncingPill: "Syncing…",
      zeroTelemetry: "Loopback RPC · Local only",
    },
    zh: {
      waiting: "等待连接 Antigravity",
      waitingDesc: "打开 Antigravity 客户端 / IDE 或接入 agy CLI 即可实时同步模型额度。",
      retry: "立即重试",
      demoPreview: "演示模式预览",
      skinStudio: "皮肤工坊",
      openFolder: "打开皮肤目录",
      gauge: "进度样式",
      accents: "强调色",
      radius: "圆角半径",
      opacity: "背景不透明",
      glow: "呼吸光晕",
      saveToml: "保存为皮肤",
      customSkinPh: "自定义皮肤名称",
      prefs: "偏好设置",
      display: "显示形态",
      pillStyle: "胶囊信息",
      position: "悬停位置",
      resetPos: "居中置顶",
      menubarText: "状态栏文字",
      syncRate: "刷新频率",
      language: "界面语言",
      alerts: "低额度提醒",
      demoMode: "演示模式",
      hide: "隐藏",
      hideTip: "隐藏灵动岛（可在托盘/状态栏重新显示）",
      collapse: "收起",
      dragHeaderTip: "按住拖拽位置 · 双击恢复居中置顶",
      pillTip: "悬停/点击展开 · 拖拽移动 · 右键隐藏",
      justNow: "刚刚更新",
      secAgo: (s) => `${s}秒前`,
      minAgo: (m) => `${m}分钟前`,
      resetIn: (t) => `${t} 后刷新`,
      resetDue: "即将刷新",
      resetAt: (tm) => `预计 ${tm}`,
      wkTag: "周",
      fiveHourName: "5小时额度",
      weeklyName: "本周额度",
      sessionName: "会话额度",
      fullTag: "满额",
      fullQuota: "额度充足",
      weekdays: ["周日", "周一", "周二", "周三", "周四", "周五", "周六"],
      offlinePill: "未连接 AGY",
      syncingPill: "同步中…",
      zeroTelemetry: "纯本地回环同步 · 零上传",
    },
  };

  let payload = null;
  let expanded = false;
  let activeDrawer = null; // null | "skins" | "settings"
  let hoverTimer = null;
  let leaveTimer = null;
  let studioDraft = null;
  let lastWindowSize = { width: 0, height: 0, expanded: false };

  const $ = (id) => document.getElementById(id);

  function post(cmd, extra = {}) {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify({ cmd, ...extra }));
    }
  }

  function langPack() {
    const langPref = payload?.config?.language || "auto";
    if (langPref === "zh") return I18N.zh;
    if (langPref === "en") return I18N.en;
    const nav = (navigator.language || "en").toLowerCase();
    return nav.startsWith("zh") ? I18N.zh : I18N.en;
  }

  function applyTexts() {
    const t = langPack();
    $("offline-title").textContent = t.waiting;
    $("offline-desc").textContent = t.waitingDesc;
    $("btn-retry").textContent = t.retry;
    $("btn-demo-preview").textContent = t.demoPreview;
    $("skin-studio-title").textContent = t.skinStudio;
    $("btn-open-skins-dir").textContent = t.openFolder;
    $("lbl-gauge").textContent = t.gauge;
    $("lbl-accents").textContent = t.accents;
    $("lbl-radius").textContent = t.radius;
    $("lbl-opacity").textContent = t.opacity;
    $("lbl-glow").textContent = t.glow;
    $("btn-save-skin").textContent = t.saveToml;
    $("input-skin-name").placeholder = t.customSkinPh;
    $("settings-title").textContent = t.prefs;
    $("lbl-mode").textContent = t.display;
    $("lbl-density").textContent = t.pillStyle;
    $("lbl-position").textContent = t.position;
    $("btn-reset-pos").textContent = t.resetPos;
    $("lbl-menubar-text").textContent = t.menubarText;
    $("lbl-poll").textContent = t.syncRate;
    $("lbl-lang").textContent = t.language;
    $("lbl-notify").textContent = t.alerts;
    $("lbl-demo").textContent = t.demoMode;
    $("btn-hide").title = t.hideTip;
    $("btn-hide-footer").textContent = t.hide;
    $("btn-collapse").textContent = t.collapse;
    $("drag-header").title = payload?.config?.mode === "notch_hug" ? "" : t.dragHeaderTip;
    $("pill-view").title = payload?.config?.mode === "notch_hug" ? "" : t.pillTip;
  }

  function applySkinVars(skin) {
    if (!skin) return;
    const rootStyle = document.documentElement.style;
    const p = skin.palette || {};
    const g = skin.geometry || {};
    const s = skin.style || {};

    rootStyle.setProperty("--bg", p.bg || "rgba(10,11,15,0.94)");
    rootStyle.setProperty("--bg-expanded", p.bg_expanded || "rgba(12,13,18,0.96)");
    rootStyle.setProperty("--surface", p.surface || "rgba(255,255,255,0.055)");
    rootStyle.setProperty("--border", p.border || "rgba(255,255,255,0.12)");
    rootStyle.setProperty("--text-primary", p.text_primary || "#f8fafc");
    rootStyle.setProperty("--text-secondary", p.text_secondary || "#cbd5e1");
    rootStyle.setProperty("--text-muted", p.text_muted || "#7c889b");
    rootStyle.setProperty("--accent-gemini", p.accent_gemini || "#38bdf8");
    rootStyle.setProperty("--accent-claude", p.accent_claude || "#fb923c");
    rootStyle.setProperty("--accent-argon", p.accent_argon || "#c084fc");
    rootStyle.setProperty("--ok-color", p.ok_color || "#34d399");
    rootStyle.setProperty("--warn-color", p.warn_color || "#fbbf24");
    rootStyle.setProperty("--danger-color", p.danger_color || "#fb7185");
    rootStyle.setProperty("--track-bg", p.track_bg || "rgba(255,255,255,0.09)");

    rootStyle.setProperty("--pill-radius", `${g.pill_radius_px ?? 20}px`);
    rootStyle.setProperty("--card-radius", `${g.card_radius_px ?? 26}px`);
    rootStyle.setProperty("--border-width", `${g.border_width_px ?? 1}px`);
    rootStyle.setProperty("--blur-px", `${g.blur_px ?? 20}px`);
    rootStyle.setProperty("--glow-alpha", `${s.glow_intensity ?? 0.45}`);
    if (s.font_family) rootStyle.setProperty("--font-main", s.font_family);
    if (s.mono_font) rootStyle.setProperty("--font-mono", s.mono_font);

    const stage = $("root");
    stage.dataset.glow = s.glow ? "true" : "false";
    stage.dataset.noise = s.noise_overlay ? "true" : "false";

    $("custom-skin-css").textContent = skin.custom_css || "";
  }

  function accentVarFor(key) {
    if (key === "gemini") return "var(--accent-gemini)";
    if (key === "claude") return "var(--accent-claude)";
    if (key === "argon") return "var(--accent-argon)";
    return "var(--ok-color)";
  }

  function healthColorForPercent(pct) {
    const warn = payload?.config?.warn_threshold ?? 30;
    const danger = payload?.config?.danger_threshold ?? 10;
    if (pct <= danger) return "var(--danger-color)";
    if (pct <= warn) return "var(--warn-color)";
    return null;
  }

  function computeHealthState(snapshot) {
    if (!snapshot) return "offline";
    let minPct = 100;
    let seen = false;
    for (const g of snapshot.groups || []) {
      for (const b of g.buckets || []) {
        if (!b.disabled && typeof b.remaining_fraction === "number") {
          seen = true;
          const p = Math.round(Math.max(0, Math.min(1, b.remaining_fraction)) * 100);
          if (p < minPct) minPct = p;
        }
      }
    }
    if (!seen) return "ok";
    const warn = payload?.config?.warn_threshold ?? 30;
    const danger = payload?.config?.danger_threshold ?? 10;
    if (minPct <= danger) return "danger";
    if (minPct <= warn) return "warn";
    return "ok";
  }

  function formatCountdown(isoString) {
    const t = langPack();
    if (!isoString) return "";
    const targetMs = Date.parse(isoString);
    if (!Number.isFinite(targetMs)) return "";
    const diffSec = Math.floor((targetMs - Date.now()) / 1000);
    if (diffSec <= 0) return t.resetDue;

    const days = Math.floor(diffSec / 86400);
    const hours = Math.floor((diffSec % 86400) / 3600);
    const mins = Math.floor((diffSec % 3600) / 60);
    const secs = diffSec % 60;
    const pad = (n) => String(n).padStart(2, "0");

    if (days > 0) return t.resetIn(`${days}d ${hours}h`);
    if (hours > 0) return t.resetIn(`${hours}h ${pad(mins)}m`);
    return t.resetIn(`${mins}m ${pad(secs)}s`);
  }

  function formatSyncAge(isoString) {
    const t = langPack();
    if (!isoString) return "--";
    const ms = Date.parse(isoString);
    if (!Number.isFinite(ms)) return "--";
    const sec = Math.max(0, Math.floor((Date.now() - ms) / 1000));
    if (sec < 5) return t.justNow;
    if (sec < 60) return t.secAgo(sec);
    return t.minAgo(Math.floor(sec / 60));
  }

  function computePaceDelta(bucket) {
    if (typeof bucket.remaining_fraction !== "number" || !bucket.reset_time) return null;
    const winMins = bucket.window_minutes || (bucket.cadence?.kind === "weekly" ? 10080 : 300);
    const targetMs = Date.parse(bucket.reset_time);
    const secsLeft = (targetMs - Date.now()) / 1000;
    if (!Number.isFinite(secsLeft) || secsLeft <= 0) return null;
    const expected = Math.max(0, Math.min(1, secsLeft / (winMins * 60))) * 100;
    const actual = Math.max(0, Math.min(1, bucket.remaining_fraction)) * 100;
    return Math.round(actual - expected);
  }

  function formatShortTimer(isoString) {
    if (!isoString) return "";
    const targetMs = Date.parse(isoString);
    if (!Number.isFinite(targetMs)) return "";
    const diffSec = Math.floor((targetMs - Date.now()) / 1000);
    if (diffSec <= 0) return "0m";
    const days = Math.floor(diffSec / 86400);
    const hours = Math.floor((diffSec % 86400) / 3600);
    const mins = Math.floor((diffSec % 3600) / 60);
    const secs = diffSec % 60;
    if (days > 0) return `${days}d${hours}h`;
    if (hours > 0) return `${hours}h${String(mins).padStart(2, "0")}m`;
    if (mins >= 10) return `${mins}m`;
    return `${mins}m${String(secs).padStart(2, "0")}s`;
  }

  function isResetSoon(isoString) {
    if (!isoString) return false;
    const targetMs = Date.parse(isoString);
    if (!Number.isFinite(targetMs)) return false;
    const diffSec = Math.floor((targetMs - Date.now()) / 1000);
    return diffSec > 0 && diffSec <= 45 * 60;
  }

  function formatExactResetTime(isoString, isWeekly) {
    if (!isoString) return "";
    const targetMs = Date.parse(isoString);
    if (!Number.isFinite(targetMs)) return "";
    const d = new Date(targetMs);
    const hh = String(d.getHours()).padStart(2, "0");
    const mm = String(d.getMinutes()).padStart(2, "0");
    const diffHrs = (targetMs - Date.now()) / 3600000;
    const t = langPack();
    if (isWeekly || diffHrs >= 20) {
      const wd = t.weekdays[d.getDay()] || "";
      return t.resetAt(`${wd} ${hh}:${mm}`);
    }
    return t.resetAt(`${hh}:${mm}`);
  }

  function localizeBucketName(bucket) {
    const t = langPack();
    const kind = bucket.cadence?.kind;
    if (kind === "five_hour") return t.fiveHourName;
    if (kind === "weekly") return t.weeklyName;
    if (kind === "session") return t.sessionName;
    return bucket.display_name || "Quota";
  }

  function extractGlanceGroups(snapshot) {
    if (!snapshot) return [];
    const list = [];
    for (const g of snapshot.groups || []) {
      const active = (g.buckets || []).filter(
        (b) => !b.disabled && typeof b.remaining_fraction === "number"
      );
      if (active.length === 0) continue;

      const pctOf = (b) => Math.round(Math.max(0, Math.min(1, b.remaining_fraction)) * 100);
      const fiveH = active.find(
        (b) => b.cadence?.kind === "five_hour" || b.cadence?.kind === "session"
      );
      const weekly = active.find((b) => b.cadence?.kind === "weekly");
      const primary = fiveH || active[0];
      const secondary = fiveH && weekly ? weekly : null;
      const minPct = Math.min(...active.map(pctOf));

      list.push({
        code:
          g.accent_key === "gemini"
            ? "G"
            : g.accent_key === "claude"
            ? "C"
            : g.accent_key === "argon"
            ? "A"
            : "M",
        shortLabel: g.short_label || g.display_name,
        accent_key: g.accent_key,
        primaryPct: pctOf(primary),
        primaryReset: primary.reset_time || null,
        primaryTag:
          primary.cadence?.kind === "five_hour"
            ? "5h"
            : primary.cadence?.kind === "weekly"
            ? langPack().wkTag
            : primary.cadence?.kind === "session"
            ? "Sess"
            : "5h",
        secondaryPct: secondary ? pctOf(secondary) : null,
        secondaryReset: secondary?.reset_time || null,
        minPct,
      });
    }
    return list.slice(0, 3);
  }

  function makePillChip(item, density, isNotch) {
    const t = langPack();
    const accent = accentVarFor(item.accent_key);
    const primaryColor = healthColorForPercent(item.primaryPct) || accent;
    const overallColor = healthColorForPercent(item.minPct) || accent;

    if (density === "compact") {
      const chip = document.createElement("span");
      chip.className = "pill-chip";
      chip.innerHTML = `
        <span class="pill-chip-code" style="color:${overallColor}">${item.code}</span>
        <span class="pill-chip-pct">${item.minPct}%</span>
      `;
      return chip;
    }

    if (isNotch) {
      const r = 7.2;
      const c = 2 * Math.PI * r;
      const dash = (c * item.primaryPct) / 100;
      const subHtml =
        item.secondaryPct !== null
          ? `<span class="glance-sub-badge">${t.wkTag}${item.secondaryPct}%</span>`
          : "";
      const chip = document.createElement("span");
      chip.className = "glance-chip";
      chip.innerHTML = `
        <span class="mini-ring" aria-hidden="true">
          <svg width="19" height="19" viewBox="0 0 20 20">
            <circle cx="10" cy="10" r="${r}" fill="none" stroke="var(--track-bg)" stroke-width="2.4"/>
            <circle cx="10" cy="10" r="${r}" fill="none" stroke="${primaryColor}" stroke-width="2.4"
              stroke-linecap="round" stroke-dasharray="${dash.toFixed(1)} ${c.toFixed(1)}"/>
          </svg>
        </span>
        <span class="glance-text">
          <span class="glance-code" style="color:${primaryColor}">${item.code}</span>
          <span class="glance-pct">${item.primaryPct}%</span>
          ${subHtml}
        </span>
      `;
      return chip;
    }

    // Dual-Row At-a-Glance Card (Concentric 5h + Weekly Dual-Ring | Row 1: 5h % + 5h Countdown | Row 2: Weekly % + Bar)
    const rOuter = 11.8;
    const cOuter = 2 * Math.PI * rOuter;
    const dashOuter = (cOuter * item.primaryPct) / 100;

    const hasWeekly = item.secondaryPct !== null;
    const wkPct = hasWeekly ? item.secondaryPct : item.primaryPct;
    const wkColor = hasWeekly
      ? healthColorForPercent(wkPct) || `color-mix(in srgb, ${accent} 76%, var(--text-primary))`
      : primaryColor;

    const rInner = 7.7;
    const cInner = 2 * Math.PI * rInner;
    const dashInner = (cInner * wkPct) / 100;

    const timerText = formatShortTimer(item.primaryReset);
    const soon = isResetSoon(item.primaryReset) && item.primaryPct < 80;

    let timerHtml = "";
    if (timerText) {
      timerHtml = `
        <span class="timer-5h ${soon ? "soon" : ""}" data-soon-reset="${escapeHtml(item.primaryReset || "")}" data-pct="${item.primaryPct}">
          ${CLOCK_SVG}<span class="timer-5h-text" data-short-reset="${escapeHtml(item.primaryReset || "")}">${timerText}</span>
        </span>
      `;
    } else if (item.primaryPct >= 100) {
      timerHtml = `<span class="timer-5h full">${CHECK_SVG}<span>${t.fullTag}</span></span>`;
    }

    const row2Html = hasWeekly
      ? `
        <div class="glance-row-wk">
          <span class="tag-wk">${t.wkTag}</span>
          <span class="val-wk" style="${healthColorForPercent(wkPct) ? `color:${wkColor}` : ""}">${wkPct}%</span>
          <span class="wk-bar-track"><span class="wk-bar-fill" style="width:${wkPct}%;background:${wkColor}"></span></span>
        </div>
      `
      : `
        <div class="glance-row-wk">
          <span class="tag-wk">${escapeHtml(item.shortLabel.slice(0, 6))}</span>
          <span class="wk-bar-track"><span class="wk-bar-fill" style="width:${item.primaryPct}%;background:${primaryColor}"></span></span>
        </div>
      `;

    const card = document.createElement("span");
    card.className = "glance-card";
    card.innerHTML = `
      <span class="dual-ring" aria-hidden="true">
        <svg width="28" height="28" viewBox="0 0 28 28">
          <circle cx="14" cy="14" r="${rOuter}" fill="none" stroke="var(--track-bg)" stroke-width="2.4"/>
          <circle cx="14" cy="14" r="${rOuter}" fill="none" stroke="${primaryColor}" stroke-width="2.4"
            stroke-linecap="round" stroke-dasharray="${dashOuter.toFixed(1)} ${cOuter.toFixed(1)}"/>
          ${
            hasWeekly
              ? `
            <circle cx="14" cy="14" r="${rInner}" fill="none" stroke="var(--track-bg)" stroke-width="1.8" opacity="0.8"/>
            <circle cx="14" cy="14" r="${rInner}" fill="none" stroke="${wkColor}" stroke-width="1.8"
              stroke-linecap="round" stroke-dasharray="${dashInner.toFixed(1)} ${cInner.toFixed(1)}"/>
          `
              : ""
          }
        </svg>
        <span class="dual-ring-code" style="color:${primaryColor}">${item.code}</span>
      </span>
      <span class="glance-rows">
        <div class="glance-row-5h">
          <span class="tag-5h" style="color:${primaryColor};background:color-mix(in srgb, ${primaryColor} 18%, transparent)">${escapeHtml(item.primaryTag)}</span>
          <span class="val-5h" style="${healthColorForPercent(item.primaryPct) ? `color:${primaryColor}` : ""}">${item.primaryPct}%</span>
          ${timerHtml}
        </div>
        ${row2Html}
      </span>
    `;
    return card;
  }

  function renderGauge(bucket, accent, gaugeStyle) {
    const t = langPack();
    const pct = bucket.disabled
      ? 0
      : typeof bucket.remaining_fraction === "number"
      ? Math.round(Math.max(0, Math.min(1, bucket.remaining_fraction)) * 100)
      : 0;
    const fillCol = bucket.disabled ? "var(--text-muted)" : (healthColorForPercent(pct) || accent);
    const isFiveHour = bucket.cadence?.kind === "five_hour" || bucket.cadence?.kind === "session";
    const isWeekly = bucket.cadence?.kind === "weekly";
    const cadenceTag = isFiveHour
      ? bucket.cadence?.kind === "session"
        ? "Sess"
        : "5h"
      : isWeekly
      ? t.wkTag
      : bucket.cadence?.label || "Limit";
    const cadenceStyle = isFiveHour
      ? `style="color:${accent};background:color-mix(in srgb, ${accent} 17%, transparent)"`
      : "";
    const cadenceClass = isFiveHour ? "cadence-5h" : isWeekly ? "cadence-wk" : "";
    const localizedName = localizeBucketName(bucket);

    const pace = computePaceDelta(bucket);
    const paceHtml =
      pace !== null && Math.abs(pace) >= 3
        ? `<span class="pace-tag ${pace >= 0 ? "ahead" : "fast"}">${pace > 0 ? "+" : ""}${pace}%</span>`
        : "";

    const countdown = formatCountdown(bucket.reset_time);
    const exactClock = formatExactResetTime(bucket.reset_time, isWeekly);
    const soon = isFiveHour && isResetSoon(bucket.reset_time) && pct < 80;

    let footHtml = "";
    if (countdown) {
      footHtml = `
        <div class="bucket-foot">
          <span class="reset-pill ${isFiveHour ? "is-5h" : ""} ${soon ? "soon" : ""}" data-soon-reset="${escapeHtml(bucket.reset_time || "")}" data-pct="${pct}">
            ${CLOCK_SVG}<span class="reset-pill-text" data-reset="${escapeHtml(bucket.reset_time || "")}">${countdown}</span>
          </span>
          ${exactClock ? `<span class="reset-clock" data-clock-reset="${escapeHtml(bucket.reset_time || "")}" data-weekly="${isWeekly ? "1" : "0"}">${escapeHtml(exactClock)}</span>` : ""}
        </div>
      `;
    } else if (!bucket.disabled && pct >= 100) {
      footHtml = `
        <div class="bucket-foot">
          <span class="reset-pill full">${CHECK_SVG}<span>${t.fullQuota}</span></span>
        </div>
      `;
    }

    if (gaugeStyle === "ring") {
      const r = 15;
      const c = 2 * Math.PI * r;
      const dash = (c * pct) / 100;
      return `
        <div class="ring-row">
          <div class="ring-wrap">
            <svg width="38" height="38" viewBox="0 0 38 38">
              <circle cx="19" cy="19" r="${r}" fill="none" stroke="var(--track-bg)" stroke-width="3.4"/>
              <circle cx="19" cy="19" r="${r}" fill="none" stroke="${fillCol}" stroke-width="3.4"
                stroke-linecap="round" stroke-dasharray="${dash.toFixed(1)} ${c.toFixed(1)}"/>
            </svg>
            <div class="ring-center" style="color:${fillCol}">${bucket.disabled ? "--" : `${pct}%`}</div>
          </div>
          <div class="ring-meta">
            <div class="bucket-top">
              <span class="bucket-name-wrap">
                <span class="cadence-pill ${cadenceClass}" ${cadenceStyle}>${escapeHtml(cadenceTag)}</span>
                <span class="bucket-label">${escapeHtml(localizedName)}</span>
              </span>
              <span class="bucket-stats">
                ${paceHtml}
              </span>
            </div>
            ${footHtml}
          </div>
        </div>
      `;
    }

    let trackHtml = "";
    if (gaugeStyle === "segments") {
      const filled = Math.round((pct / 100) * 12);
      const cells = Array.from({ length: 12 }, (_, idx) => {
        const on = !bucket.disabled && idx < filled;
        return `<span class="seg-cell ${on ? "filled" : ""}" style="${on ? `background:${fillCol};color:${fillCol}` : ""}"></span>`;
      }).join("");
      trackHtml = `<div class="segments-track">${cells}</div>`;
    } else if (gaugeStyle === "wave") {
      trackHtml = `
        <div class="wave-track">
          <div class="wave-fill" style="width:${pct}%;background:${fillCol}">
            <svg viewBox="0 0 120 10" preserveAspectRatio="none">
              <path d="M0 5 Q 15 0, 30 5 T 60 5 T 90 5 T 120 5 L120 10 L0 10 Z" fill="rgba(255,255,255,0.35)"/>
            </svg>
          </div>
        </div>
      `;
    } else {
      trackHtml = `
        <div class="bar-track">
          <div class="bar-fill" style="width:${pct}%;background:${fillCol}"></div>
        </div>
      `;
    }

    return `
      <div class="bucket-row">
        <div class="bucket-top">
          <span class="bucket-name-wrap">
            <span class="cadence-pill ${cadenceClass}" ${cadenceStyle}>${escapeHtml(cadenceTag)}</span>
            <span class="bucket-label">${escapeHtml(localizedName)}</span>
          </span>
          <span class="bucket-stats">
            ${paceHtml}
            <span class="pct-text" style="color:${fillCol}">${bucket.disabled ? "--" : `${pct}%`}</span>
          </span>
        </div>
        ${trackHtml}
        ${footHtml}
      </div>
    `;
  }

  function render() {
    if (!payload) return;
    applyTexts();

    const activeSkin = studioDraft || payload.skin;
    applySkinVars(activeSkin);

    const stage = $("root");
    const mode = payload.config?.mode || "island";
    const density = payload.config?.pill_density || "glance";
    stage.className = `stage mode-${mode} density-${density}`;
    stage.dataset.expanded = expanded ? "true" : "false";

    const notchGap =
      mode === "notch_hug" && !expanded
        ? Math.max(132, Math.round(payload.screen?.notch_width || 164))
        : 0;
    document.documentElement.style.setProperty("--notch-gap", `${notchGap}px`);

    const sync = payload.status || {};
    const snap = sync.snapshot || null;
    stage.dataset.health = computeHealthState(snap);

    $("pill-sync-spin").classList.toggle("hidden", !sync.syncing);
    $("btn-refresh").classList.toggle("spinning", !!sync.syncing);
    $("btn-pin").classList.toggle("active", !!payload.config?.pinned_expanded);
    $("btn-skins").classList.toggle("active", activeDrawer === "skins");
    $("btn-settings").classList.toggle("active", activeDrawer === "settings");

    // Collapsed pill rendering (At-a-Glance on both macOS & Windows)
    const leftBox = $("pill-metrics-left");
    const rightBox = $("pill-metrics-right");
    leftBox.innerHTML = "";
    rightBox.innerHTML = "";

    const glanceGroups = extractGlanceGroups(snap);
    const ambientFill = $("pill-ambient-fill");
    if (glanceGroups.length === 0) {
      ambientFill.style.width = "0%";
      const span = document.createElement("span");
      span.className = "pill-offline-label";
      span.textContent = sync.syncing ? langPack().syncingPill : langPack().offlinePill;
      leftBox.appendChild(span);
    } else {
      const tightestPct = Math.min(...glanceGroups.map((g) => g.minPct));
      ambientFill.style.width = `${tightestPct}%`;

      if (mode === "notch_hug") {
        leftBox.appendChild(makePillChip(glanceGroups[0], density, true));
        for (const g of glanceGroups.slice(1)) {
          rightBox.appendChild(makePillChip(g, density, true));
        }
      } else {
        glanceGroups.forEach((g, idx) => {
          if (idx > 0 && density === "glance") {
            const sep = document.createElement("span");
            sep.className = "pill-divider";
            leftBox.appendChild(sep);
          }
          leftBox.appendChild(makePillChip(g, density, false));
        });
      }
    }

    $("pill-view").classList.toggle("hidden", expanded);
    $("expanded-view").classList.toggle("hidden", !expanded);

    if (expanded) {
      const sourceLabelMap = {
        app_local: "App RPC",
        ide_local: "IDE RPC",
        cli_local: "CLI RPC",
        cli_print_usage: "agy /usage",
        cli_status_line: "StatusLine",
        demo: "Simulated",
      };

      $("plan-badge").textContent = snap?.account_plan || "Antigravity";
      $("demo-badge").classList.toggle("hidden", !sync.demo_mode);
      $("source-label").textContent = snap ? sourceLabelMap[snap.source] || snap.source : "Offline";
      $("sync-age").textContent = snap ? formatSyncAge(snap.fetched_at) : "--";

      if (snap && typeof snap.ai_credits === "number") {
        $("credits-label").textContent = `· ${Math.round(snap.ai_credits)} cr`;
        $("credits-label").classList.remove("hidden");
      } else {
        $("credits-label").classList.add("hidden");
      }

      const groupsEl = $("groups-container");
      const offlineEl = $("offline-card");
      if (snap && (snap.groups || []).length > 0) {
        offlineEl.classList.add("hidden");
        groupsEl.classList.remove("hidden");
        const gaugeStyle = activeSkin?.style?.gauge || "ring";
        groupsEl.innerHTML = snap.groups
          .map((g) => {
            const accent = accentVarFor(g.accent_key);
            const bucketsHtml = (g.buckets || [])
              .map((b) => renderGauge(b, accent, gaugeStyle))
              .join("");
            return `
              <section class="group-card">
                <div class="group-header">
                  <span class="group-title">
                    <span class="group-dot" style="background:${accent}"></span>
                    ${escapeHtml(g.display_name)}
                  </span>
                </div>
                ${bucketsHtml}
              </section>
            `;
          })
          .join("");
      } else {
        groupsEl.classList.add("hidden");
        offlineEl.classList.remove("hidden");
      }

      $("account-label").textContent = snap?.account_email || langPack().zeroTelemetry;

      renderDrawers(activeSkin);
    }

    requestResize();
  }

  function renderDrawers(activeSkin) {
    $("drawer-skins").classList.toggle("hidden", activeDrawer !== "skins");
    $("drawer-settings").classList.toggle("hidden", activeDrawer !== "settings");

    if (activeDrawer === "skins") {
      const grid = $("skin-grid");
      const allSkins = payload.all_skins || [];
      grid.innerHTML = allSkins
        .map((s) => {
          const isActive = !studioDraft && s.id === payload.skin?.id;
          return `
            <button class="skin-card ${isActive ? "active" : ""}" data-skin-id="${escapeHtml(s.id)}">
              <div class="skin-swatches">
                <span class="swatch" style="background:${s.palette.bg}"></span>
                <span class="swatch" style="background:${s.palette.accent_gemini}"></span>
                <span class="swatch" style="background:${s.palette.accent_claude}"></span>
              </div>
              <span class="skin-name">${escapeHtml(s.name)}</span>
            </button>
          `;
        })
        .join("");

      grid.querySelectorAll(".skin-card").forEach((btn) => {
        btn.onclick = () => {
          studioDraft = null;
          post("set_skin", { id: btn.dataset.skinId });
        };
      });

      const cur = activeSkin;
      $("gauge-selector")
        .querySelectorAll("button")
        .forEach((b) => b.classList.toggle("active", b.dataset.gauge === cur.style.gauge));
      $("pick-gemini").value = toHex(cur.palette.accent_gemini, "#38bdf8");
      $("pick-claude").value = toHex(cur.palette.accent_claude, "#fb923c");
      $("pick-argon").value = toHex(cur.palette.accent_argon, "#c084fc");
      $("range-radius").value = cur.geometry.card_radius_px ?? 26;
      $("val-radius").textContent = `${cur.geometry.card_radius_px ?? 26}px`;
      const alpha = extractAlphaPct(cur.palette.bg);
      $("range-opacity").value = alpha;
      $("val-opacity").textContent = `${alpha}%`;
      $("chk-glow").checked = !!cur.style.glow;
    }

    if (activeDrawer === "settings") {
      const cfg = payload.config || {};
      const isMac = payload.platform === "macos";
      $("btn-mode-notch").classList.toggle("hidden", !isMac);
      $("row-position").classList.toggle("hidden", cfg.mode === "notch_hug");
      $("row-menubar-text").classList.toggle("hidden", !isMac);
      $("chk-menubar-text").checked = !!cfg.always_show_menubar_text;
      $("mode-selector")
        .querySelectorAll("button")
        .forEach((b) => b.classList.toggle("active", b.dataset.mode === cfg.mode));
      $("density-selector")
        .querySelectorAll("button")
        .forEach((b) => b.classList.toggle("active", b.dataset.density === (cfg.pill_density || "glance")));
      $("poll-selector")
        .querySelectorAll("button")
        .forEach((b) => b.classList.toggle("active", Number(b.dataset.secs) === cfg.poll_interval_secs));
      $("lang-selector")
        .querySelectorAll("button")
        .forEach((b) => b.classList.toggle("active", b.dataset.lang === cfg.language));
      $("chk-notify").checked = !!cfg.notifications;
      $("chk-demo").checked = !!payload.status?.demo_mode;
    }
  }

  function ensureDraft() {
    if (!studioDraft && payload?.skin) {
      studioDraft = JSON.parse(JSON.stringify(payload.skin));
    }
    return studioDraft;
  }

  function requestResize() {
    requestAnimationFrame(() => {
      const stage = $("root");
      const rect = stage.getBoundingClientRect();
      const width = Math.ceil(Math.max(rect.width, stage.scrollWidth));
      const height = Math.ceil(Math.max(rect.height, stage.scrollHeight));
      if (
        width > 0 &&
        height > 0 &&
        (Math.abs(width - lastWindowSize.width) > 1 ||
          Math.abs(height - lastWindowSize.height) > 1 ||
          expanded !== lastWindowSize.expanded)
      ) {
        lastWindowSize = { width, height, expanded };
        post("resize", { width, height, expanded });
      }
    });
  }

  function setExpanded(next) {
    if (expanded === next) return;
    expanded = next;
    if (!expanded) {
      activeDrawer = null;
      studioDraft = null;
    }
    render();
  }

  // Hover + Click + Universal Drag & Hide Interactions (macOS & Windows)
  const shell = $("island-shell");
  let pillPress = null;
  let suppressPillClick = false;
  let suppressResetTimer = null;

  shell.addEventListener("mouseenter", () => {
    clearTimeout(leaveTimer);
    if (!expanded && !pillPress && payload?.config?.mode !== "widget") {
      hoverTimer = setTimeout(() => {
        if (!pillPress) setExpanded(true);
      }, 180);
    }
  });

  shell.addEventListener("mouseleave", () => {
    clearTimeout(hoverTimer);
    pillPress = null;
    if (
      expanded &&
      !payload?.config?.pinned_expanded &&
      !activeDrawer &&
      payload?.config?.mode !== "widget"
    ) {
      leaveTimer = setTimeout(() => setExpanded(false), 260);
    }
  });

  $("pill-view").addEventListener("mousedown", (e) => {
    clearTimeout(hoverTimer);
    if (payload?.config?.mode !== "notch_hug" && e.button === 0) {
      pillPress = { x: e.clientX, y: e.clientY };
      suppressPillClick = false;
    }
  });
  window.addEventListener("mousemove", (e) => {
    if (!pillPress) return;
    if (Math.hypot(e.clientX - pillPress.x, e.clientY - pillPress.y) > 4) {
      pillPress = null;
      suppressPillClick = true;
      clearTimeout(suppressResetTimer);
      suppressResetTimer = setTimeout(() => {
        suppressPillClick = false;
      }, 350);
      clearTimeout(hoverTimer);
      post("drag");
    }
  });
  window.addEventListener("mouseup", () => {
    pillPress = null;
  });
  window.addEventListener("blur", () => {
    pillPress = null;
  });
  $("pill-view").addEventListener("click", () => {
    if (suppressPillClick) {
      suppressPillClick = false;
      return;
    }
    setExpanded(true);
  });
  $("pill-view").addEventListener("contextmenu", (e) => {
    e.preventDefault();
    post("hide");
  });

  $("drag-header").addEventListener("mousedown", (e) => {
    if (e.target.closest("button") || e.button !== 0) return;
    if (payload?.config?.mode !== "notch_hug") {
      post("drag");
    }
  });
  $("drag-header").addEventListener("dblclick", (e) => {
    if (e.target.closest("button")) return;
    if (payload?.config?.mode !== "notch_hug") {
      post("reset_position");
    }
  });

  $("btn-hide").addEventListener("click", () => post("hide"));
  $("btn-hide-footer").addEventListener("click", () => post("hide"));
  $("btn-reset-pos").addEventListener("click", () => post("reset_position"));

  $("btn-collapse").addEventListener("click", () => {
    if (payload?.config?.pinned_expanded) {
      post("set_pinned", { pinned: false });
    }
    setExpanded(false);
  });

  $("btn-pin").addEventListener("click", () => {
    const next = !payload?.config?.pinned_expanded;
    post("set_pinned", { pinned: next });
  });

  $("btn-refresh").addEventListener("click", () => post("refresh"));
  $("btn-retry").addEventListener("click", () => post("refresh"));
  $("btn-demo-preview").addEventListener("click", () => post("set_demo", { enabled: true }));

  $("btn-mode").addEventListener("click", () => {
    const cur = payload?.config?.mode || "island";
    const next = cur === "widget" ? "island" : "widget";
    post("set_mode", { mode: next });
  });

  $("btn-skins").addEventListener("click", () => {
    activeDrawer = activeDrawer === "skins" ? null : "skins";
    render();
  });

  $("btn-settings").addEventListener("click", () => {
    activeDrawer = activeDrawer === "settings" ? null : "settings";
    render();
  });

  $("btn-open-skins-dir").addEventListener("click", () => post("open_skins_dir"));

  // Skin Studio Live Bindings
  $("gauge-selector").addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-gauge]");
    if (!btn) return;
    const d = ensureDraft();
    if (!d) return;
    d.style.gauge = btn.dataset.gauge;
    render();
  });

  $("pick-gemini").addEventListener("input", (e) => {
    const d = ensureDraft();
    if (!d) return;
    d.palette.accent_gemini = e.target.value;
    render();
  });
  $("pick-claude").addEventListener("input", (e) => {
    const d = ensureDraft();
    if (!d) return;
    d.palette.accent_claude = e.target.value;
    render();
  });
  $("pick-argon").addEventListener("input", (e) => {
    const d = ensureDraft();
    if (!d) return;
    d.palette.accent_argon = e.target.value;
    render();
  });

  $("range-radius").addEventListener("input", (e) => {
    const d = ensureDraft();
    if (!d) return;
    const r = Number(e.target.value);
    d.geometry.card_radius_px = r;
    d.geometry.pill_radius_px = Math.max(8, Math.min(24, r - 4));
    render();
  });

  $("range-opacity").addEventListener("input", (e) => {
    const d = ensureDraft();
    if (!d) return;
    const alpha = (Number(e.target.value) / 100).toFixed(2);
    d.palette.bg = replaceAlpha(d.palette.bg, alpha);
    d.palette.bg_expanded = replaceAlpha(d.palette.bg_expanded, alpha);
    render();
  });

  $("chk-glow").addEventListener("change", (e) => {
    const d = ensureDraft();
    if (!d) return;
    d.style.glow = e.target.checked;
    render();
  });

  $("btn-save-skin").addEventListener("click", () => {
    const d = ensureDraft();
    if (!d) return;
    const rawName = $("input-skin-name").value.trim() || `${d.name} Custom`;
    d.name = rawName;
    d.id = rawName.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "custom-skin";
    d.author = "User";
    studioDraft = null;
    $("input-skin-name").value = "";
    post("save_skin", { skin: d });
  });

  // Settings Bindings
  $("mode-selector").addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-mode]");
    if (btn) post("set_mode", { mode: btn.dataset.mode });
  });
  $("density-selector").addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-density]");
    if (btn) post("set_pill_density", { density: btn.dataset.density });
  });
  $("poll-selector").addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-secs]");
    if (btn) post("set_poll_interval", { secs: Number(btn.dataset.secs) });
  });
  $("lang-selector").addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-lang]");
    if (btn) post("set_language", { lang: btn.dataset.lang });
  });
  $("chk-menubar-text").addEventListener("change", (e) => {
    post("set_always_show_menubar_text", { enabled: e.target.checked });
  });
  $("chk-notify").addEventListener("change", (e) => {
    post("set_notifications", { enabled: e.target.checked });
  });
  $("chk-demo").addEventListener("change", (e) => {
    post("set_demo", { enabled: e.target.checked });
  });

  // 1-second live countdown & sync age ticker (both collapsed Glance Pill and expanded card)
  setInterval(() => {
    if (!payload?.status?.snapshot) return;
    if (expanded) {
      $("sync-age").textContent = formatSyncAge(payload.status.snapshot.fetched_at);
      document.querySelectorAll(".reset-pill-text[data-reset]").forEach((el) => {
        if (el.dataset.reset) {
          el.textContent = formatCountdown(el.dataset.reset);
        }
      });
      document.querySelectorAll(".reset-clock[data-clock-reset]").forEach((el) => {
        if (el.dataset.clockReset) {
          el.textContent = formatExactResetTime(el.dataset.clockReset, el.dataset.weekly === "1");
        }
      });
    } else {
      document.querySelectorAll(".timer-5h-text[data-short-reset]").forEach((el) => {
        if (el.dataset.shortReset) {
          el.textContent = formatShortTimer(el.dataset.shortReset);
        }
      });
    }
    document.querySelectorAll("[data-soon-reset]").forEach((el) => {
      const iso = el.dataset.soonReset;
      const pct = Number(el.dataset.pct ?? 100);
      if (iso) {
        el.classList.toggle("soon", isResetSoon(iso) && pct < 80);
      }
    });
  }, 1000);

  // Incoming state from Rust
  window.__AGY_RECEIVE__ = (next) => {
    payload = next;
    if (next.config?.pinned_expanded) {
      expanded = true;
    }
    if (typeof next.force_expanded === "boolean") {
      expanded = next.force_expanded;
    }
    render();
  };

  function escapeHtml(str) {
    return String(str)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function toHex(color, fallback) {
    if (typeof color === "string" && /^#[0-9a-f]{6}$/i.test(color.trim())) {
      return color.trim();
    }
    return fallback;
  }

  function extractAlphaPct(rgba) {
    const m = String(rgba).match(/rgba?\([^,]+,[^,]+,[^,]+,\s*([0-9.]+)\s*\)/i);
    if (m) {
      return Math.round(Math.max(0.45, Math.min(1, parseFloat(m[1]))) * 100);
    }
    return 94;
  }

  function replaceAlpha(color, alphaStr) {
    const m = String(color).match(/rgba?\(([^,]+,[^,]+,[^,]+)(?:,\s*[0-9.]+)?\)/i);
    if (m) {
      return `rgba(${m[1]}, ${alphaStr})`;
    }
    return `rgba(12, 13, 18, ${alphaStr})`;
  }

  post("ready");
})();
