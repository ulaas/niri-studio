// niri studio landing page: scroll effects, the tour, the theme picker. No dependencies.
(() => {
  "use strict";
  const root = document.documentElement;
  root.classList.add("js");
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const $ = (s, el = document) => el.querySelector(s);
  const $$ = (s, el = document) => [...el.querySelectorAll(s)];

  // ---- reveal on scroll, staggered within a section ----
  const io = new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      e.target.classList.add("in");
      io.unobserve(e.target);
    }
  }, { rootMargin: "0px 0px -8% 0px", threshold: 0.12 });
  for (const section of $$("section, header.section-head")) {
    $$(".reveal", section).forEach((el, i) => el.style.setProperty("--d", `${Math.min(i, 6) * 0.08}s`));
  }
  $$(".reveal").forEach((el) => io.observe(el));

  // ---- marquee: a second copy makes the loop seamless ----
  const track = $(".track");
  if (track) {
    for (const node of [...track.childNodes]) {
      const copy = node.cloneNode(true);
      if (copy.nodeType === 1) copy.setAttribute("aria-hidden", "true");
      track.appendChild(copy);
    }
  }

  // ---- scroll-linked: progress bar, nav state, hero tilt ----
  const bar = $(".progress span");
  const nav = $(".nav");
  const stage = $("[data-tilt]");
  let ticking = false;
  function onScroll() {
    ticking = false;
    const max = root.scrollHeight - innerHeight;
    bar.style.setProperty("--p", max > 0 ? (scrollY / max).toFixed(4) : "0");
    nav.classList.toggle("scrolled", scrollY > 24);
    if (stage && !reduce) {
      const r = stage.getBoundingClientRect();
      // 0 while the stage sits low on the screen, 1 once its top reaches a third of the way down.
      const t = Math.min(1, Math.max(0, (innerHeight - r.top) / (innerHeight * 0.75)));
      stage.style.setProperty("--t", t.toFixed(4));
    }
  }
  addEventListener("scroll", () => { if (!ticking) { ticking = true; requestAnimationFrame(onScroll); } }, { passive: true });
  addEventListener("resize", onScroll);
  onScroll();

  // ---- active nav link ----
  const links = new Map($$(".nav nav a").map((a) => [a.getAttribute("href").slice(1), a]));
  const navIo = new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (e.isIntersecting) links.forEach((a, id) => a.classList.toggle("active", id === e.target.id));
    }
  }, { rootMargin: "-45% 0px -50% 0px" });
  links.forEach((_, id) => { const s = document.getElementById(id); if (s) navIo.observe(s); });

  // ---- tour: the step in the middle of the screen picks the screenshot ----
  const shots = new Map($$(".swap img").map((img) => [img.dataset.name, img]));
  const steps = $$(".step");
  const tourIo = new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      steps.forEach((s) => s.classList.toggle("active", s === e.target));
      shots.forEach((img, name) => img.classList.toggle("on", name === e.target.dataset.shot));
    }
  }, { rootMargin: "-48% 0px -48% 0px" });
  steps.forEach((s) => tourIo.observe(s));
  if (steps[0]) steps[0].classList.add("active");

  // ---- theme picker: recolors this page with the app's palettes ----
  const THEMES = {
    "studio-dark":      { bg: "#0b0b10", bg2: "#111116", surface: "#17171d", border: "#26262e", text: "#ececf1", muted: "#a1a1aa", faint: "#71717a", accent: "#7c9cff", accent2: "#c89cff" },
    "studio-light":     { bg: "#f8f8fa", bg2: "#f1f1f4", surface: "#ffffff", border: "#e2e2e7", text: "#18181b", muted: "#52525b", faint: "#9a9aa3", accent: "#4f6bed", accent2: "#8a3bc2", light: true },
    "catppuccin-mocha": { bg: "#1e1e2e", bg2: "#181825", surface: "#24243a", border: "#313244", text: "#cdd6f4", muted: "#a6adc8", faint: "#6c7086", accent: "#cba6f7", accent2: "#f5c2e7" },
    "catppuccin-latte": { bg: "#eff1f5", bg2: "#e6e9ef", surface: "#f8f9fb", border: "#ccd0da", text: "#4c4f69", muted: "#6c6f85", faint: "#9ca0b0", accent: "#8839ef", accent2: "#ea76cb", light: true },
    "tokyo-night":      { bg: "#1a1b26", bg2: "#16161e", surface: "#1f2335", border: "#292e42", text: "#c0caf5", muted: "#a9b1d6", faint: "#565f89", accent: "#7aa2f7", accent2: "#bb9af7" },
    "rose-pine":        { bg: "#191724", bg2: "#16141f", surface: "#1f1d2e", border: "#2a273f", text: "#e0def4", muted: "#908caa", faint: "#6e6a86", accent: "#c4a7e7", accent2: "#ebbcba" },
    "nord":             { bg: "#2e3440", bg2: "#2b303b", surface: "#353c4a", border: "#434c5e", text: "#eceff4", muted: "#d8dee9", faint: "#8a94a8", accent: "#88c0d0", accent2: "#b48ead" },
    "gruvbox-dark":     { bg: "#282828", bg2: "#1d2021", surface: "#302e2c", border: "#3c3836", text: "#ebdbb2", muted: "#bdae93", faint: "#7c6f64", accent: "#fe8019", accent2: "#fabd2f" },
  };
  const luminance = (hex) => {
    const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255)
      .map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  function applyTheme(name) {
    const t = THEMES[name];
    if (!t) return;
    const set = (k, v) => root.style.setProperty(k, v);
    set("--bg", t.bg); set("--bg-2", t.bg2); set("--surface", t.surface); set("--border", t.border);
    set("--text", t.text); set("--muted", t.muted); set("--faint", t.faint);
    set("--accent", t.accent); set("--accent-2", t.accent2);
    set("--on-accent", luminance(t.accent) > 0.4 ? "#101014" : "#ffffff");
    root.toggleAttribute("data-light", !!t.light);
    $('meta[name="theme-color"]').setAttribute("content", t.bg);
  }
  const themeShot = $("#theme-shot");
  for (const b of $$(".theme-picker button")) {
    const t = THEMES[b.dataset.theme];
    b.style.setProperty("--sw-accent", t.accent);
    b.style.setProperty("--sw-bg", t.bg);
    b.addEventListener("click", () => {
      $$(".theme-picker button").forEach((x) => x.setAttribute("aria-checked", String(x === b)));
      applyTheme(b.dataset.theme);
      const src = `assets/shots/${b.dataset.shot}.webp`;
      if (themeShot.getAttribute("src") === src) return;
      themeShot.classList.add("fading");
      const next = new Image();
      next.onload = () => { themeShot.src = src; themeShot.classList.remove("fading"); };
      next.src = src;
    });
  }
  // Arrow keys move between the radio buttons, as a radio group should.
  $(".theme-picker").addEventListener("keydown", (e) => {
    if (!["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"].includes(e.key)) return;
    const all = $$(".theme-picker button");
    const i = all.indexOf(document.activeElement);
    if (i < 0) return;
    e.preventDefault();
    const next = all[(i + (e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : all.length - 1)) % all.length];
    next.focus();
    next.click();
  });

  // ---- counters ----
  const countIo = new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      countIo.unobserve(e.target);
      const end = Number(e.target.dataset.count);
      if (reduce || end === 0) continue;
      const start = performance.now(), dur = 1400;
      const step = (now) => {
        const k = Math.min(1, (now - start) / dur);
        e.target.textContent = String(Math.round(end * (1 - (1 - k) ** 4)));
        if (k < 1) requestAnimationFrame(step);
      };
      e.target.textContent = "0";
      requestAnimationFrame(step);
    }
  }, { threshold: 0.6 });
  $$("[data-count]").forEach((el) => countIo.observe(el));

  // ---- pointer effects: card glow, magnetic buttons, hero mesh parallax ----
  if (!reduce && matchMedia("(pointer: fine)").matches) {
    for (const card of $$(".card")) {
      card.addEventListener("pointermove", (e) => {
        const r = card.getBoundingClientRect();
        card.style.setProperty("--x", `${e.clientX - r.left}px`);
        card.style.setProperty("--y", `${e.clientY - r.top}px`);
      });
    }
    for (const btn of $$(".magnetic")) {
      btn.addEventListener("pointermove", (e) => {
        const r = btn.getBoundingClientRect();
        btn.style.setProperty("--mx", `${(e.clientX - r.left - r.width / 2) * 0.18}px`);
        btn.style.setProperty("--my", `${(e.clientY - r.top - r.height / 2) * 0.28}px`);
      });
      btn.addEventListener("pointerleave", () => { btn.style.setProperty("--mx", "0px"); btn.style.setProperty("--my", "0px"); });
    }
    const mesh = $(".mesh");
    addEventListener("pointermove", (e) => {
      const x = (e.clientX / innerWidth - 0.5) * 30, y = (e.clientY / innerHeight - 0.5) * 30;
      mesh.style.transform = `translate(${x}px, ${y}px)`;
    }, { passive: true });
  }

  // ---- copy buttons ----
  for (const btn of $$("[data-copy]")) {
    btn.addEventListener("click", async () => {
      const text = document.getElementById(btn.dataset.copy).innerText
        .split("\n").map((l) => l.replace(/\s+#.*$/, "")).filter((l) => l && !l.startsWith("#")).join("\n");
      try {
        await navigator.clipboard.writeText(text);
        btn.textContent = "Copied";
      } catch {
        btn.textContent = "Select and copy";
      }
      btn.classList.add("done");
      setTimeout(() => { btn.textContent = "Copy"; btn.classList.remove("done"); }, 1800);
    });
  }
})();
