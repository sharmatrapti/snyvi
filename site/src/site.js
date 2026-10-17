/* snyvi.com's one script. Everything on the page works without it; this
   adds what the app's window does: the theme and accent, ⌘K over every
   page, your OS's install line, copy, the film, the sidebar's place in the
   story, and the arrival. No library, no tracking. */
(() => {
  const root = document.documentElement;
  const $ = (s, el = document) => el.querySelector(s);
  const $$ = (s, el = document) => [...el.querySelectorAll(s)];
  const store = {
    get: k => { try { return localStorage.getItem(k); } catch { return null; } },
    set: (k, v) => { try { v == null ? localStorage.removeItem(k) : localStorage.setItem(k, v); } catch {} },
  };
  let mermaidP = null;   // the diagram library, once a page with a diagram asks
  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------- the mark's faces, from FACES (build.mjs puts them here) ---------- */
  const FACES = window.SNYVI_FACES || {};
  const setFace = (svg, feel) => {
    if (!svg || !FACES[feel]) return;
    $$(".mk-face", svg).forEach(g => { g.innerHTML = FACES[feel]; });
  };

  /* ---------- themes and accents ---------- */
  const THEMES = ["paper", "ink", "parchment", "snow", "sage", "contrast", "midnight", "espresso"];
  // Each accent's pair, read from ui/app.css by build.mjs: a dot wears its
  // pair and the page's theme, and the theme's own rule derives the colour.
  const PAIRS = window.SNYVI_ACCENTS || { passion: ["#8c0f26", "#f04a63"] };
  const ACCENTS = Object.keys(PAIRS);
  const name = s => s[0].toUpperCase() + s.slice(1);
  const menu = $("#theme-menu"), themeBtn = $("#theme-btn");
  const applyTheme = (theme, accent) => {
    if (theme) { root.dataset.theme = theme; store.set("snyvi-site-theme", theme); }
    if (accent !== undefined) {
      if (accent === "passion") { delete root.dataset.accent; store.set("snyvi-site-accent", null); }
      else { root.dataset.accent = accent; store.set("snyvi-site-accent", accent); }
    }
    const t = root.dataset.theme, a = root.dataset.accent || "passion";
    if (themeBtn) $(".theme-name", themeBtn).textContent = name(t);
    $$(".theme-opt", menu).forEach(b => b.setAttribute("aria-checked", b.dataset.set === t));
    $$(".accent-opt", menu).forEach(b => b.setAttribute("aria-checked", b.dataset.set === a));
    redrawDiagrams();
  };
  if (menu) {
    $(".theme-list", menu).innerHTML = THEMES.map(t =>
      `<button class="theme-opt" role="menuitemradio" data-set="${t}"><span class="sw" data-theme="${t}"></span>${name(t)}</button>`).join("");
    $(".accents", menu).innerHTML = ACCENTS.map(a =>
      `<button class="accent-opt" role="menuitemradio" data-set="${a}" style="--accent-l:${PAIRS[a][0]};--accent-d:${PAIRS[a][1]}" aria-label="${name(a)}" title="${name(a)}"></button>`).join("");
    // The swatches wear their own theme; give each accent dot the page's theme too.
    $$(".accent-opt", menu).forEach(b => b.setAttribute("data-theme", root.dataset.theme));
    menu.addEventListener("click", e => {
      const b = e.target.closest("[data-set]");
      if (!b) return;
      if (b.classList.contains("theme-opt")) {
        applyTheme(b.dataset.set);
        $$(".accent-opt", menu).forEach(d => d.setAttribute("data-theme", b.dataset.set));
      } else applyTheme(null, b.dataset.set);
    });
  }
  const openMenu = open => {
    if (!menu) return;
    menu.hidden = !open;
    themeBtn.setAttribute("aria-expanded", open);
    if (open) ($('[aria-checked="true"]', menu) || $(".theme-opt", menu)).focus();
  };
  themeBtn?.addEventListener("click", () => openMenu(menu.hidden));
  document.addEventListener("click", e => {
    if (menu && !menu.hidden && !menu.contains(e.target) && !themeBtn.contains(e.target)) openMenu(false);
    if (root.classList.contains("open-nav") && !e.target.closest(".side, .bar")) toggleNav(false);
  });
  applyTheme(null);

  /* ---------- the sidebar as a sheet, under 1024 px ---------- */
  const navBtn = $("#nav-btn");
  const toggleNav = open => {
    root.classList.toggle("open-nav", open);
    navBtn?.setAttribute("aria-expanded", open);
  };
  navBtn?.addEventListener("click", () => toggleNav(!root.classList.contains("open-nav")));
  $$(".side a").forEach(a => a.addEventListener("click", () => toggleNav(false)));

  /* ---------- install: your OS first, copy where you pressed ---------- */
  const os = (() => {
    const p = (navigator.userAgentData?.platform || navigator.platform || navigator.userAgent).toLowerCase();
    if (p.includes("win")) return "windows";
    if (p.includes("mac") || /iphone|ipad/.test(p)) return "macos";
    return "linux";
  })();
  if (os !== "macos") $$("kbd[data-pc]").forEach(k => { k.textContent = k.dataset.pc; });
  const pickOs = (box, which) => {
    $$(".tab", box).forEach(t => t.setAttribute("aria-selected", t.dataset.os === which));
    $$(".tab", box).forEach(t => t.tabIndex = t.dataset.os === which ? 0 : -1);
    $$(".os-panel", box).forEach(p => p.hidden = p.dataset.os !== which);
  };
  $$("[data-install]").forEach(box => {
    pickOs(box, os);
    box.addEventListener("click", e => {
      const t = e.target.closest(".tab");
      if (t) { $$("[data-install]").forEach(b => pickOs(b, t.dataset.os)); }
    });
    box.addEventListener("keydown", e => {
      if (!e.target.classList.contains("tab") || !["ArrowLeft", "ArrowRight"].includes(e.key)) return;
      const tabs = $$(".tab", box), i = tabs.indexOf(e.target);
      const next = tabs[(i + (e.key === "ArrowRight" ? 1 : tabs.length - 1)) % tabs.length];
      $$("[data-install]").forEach(b => pickOs(b, next.dataset.os));
      next.focus();
    });
  });
  const copied = (btn, ok) => {
    const label = $("span", btn), was = label?.textContent;
    btn.classList.toggle("done", ok);
    if (label) label.textContent = ok ? "Copied" : "Select it";
    setTimeout(() => { btn.classList.remove("done"); if (label) label.textContent = was; }, 1600);
  };
  const copy = async (text, btn) => {
    try { await navigator.clipboard.writeText(text); copied(btn, true); }
    catch { copied(btn, false); }
  };
  document.addEventListener("click", e => {
    const b = e.target.closest(".copy");
    if (b) copy($("code", b.parentElement).innerText.replace(/^\$\s*/, "").trim(), b);
  });
  // The rail's action copies the line for the OS shown in the first install box.
  $("#act-copy")?.addEventListener("click", e => {
    const panel = $("[data-install] .os-panel:not([hidden]) code") || $("[data-install] code");
    if (panel) copy(panel.innerText.replace(/^\$\s*/, "").trim(), e.currentTarget);
  });

  /* ---------- the film ---------- */
  const film = $("#film");
  const playFilm = () => {
    if (!film) return;
    const v = $("video", film);
    if (!v.src) v.src = v.dataset.src;
    film.showModal();
    v.play().catch(() => {});
  };
  $$("[data-film]").forEach(b => b.addEventListener("click", playFilm));
  film?.addEventListener("close", () => $("video", film).pause());
  $("#film-close")?.addEventListener("click", () => film.close());

  /* ---------- the clips: fetched as they come near, played while seen ----------
     The hidden theme's video never intersects, so only the one shown loads.
     With motion off they stay their poster, the step's result. */
  const clipFigs = $$("[data-clip]");
  if (reduced || !("IntersectionObserver" in window)) clipFigs.forEach(f => f.classList.add("still"));
  else if (clipFigs.length) {
    const near = new IntersectionObserver(es => es.forEach(({ target: v, isIntersecting: on }) => {
      if (on && !v.src) v.src = v.dataset.src;
      if (on && v.closest("[data-clip]").dataset.paused === undefined) v.play().catch(() => {});
      else v.pause();
    }), { rootMargin: "120px 0px" });
    clipFigs.forEach(f => {
      // A browser without H.264 keeps the poster, and nothing to pause.
      $$("video", f).forEach(v => { near.observe(v); v.addEventListener("error", () => f.classList.add("still")); });
      $(".clip-btn", f).addEventListener("click", e => {
        const stop = f.dataset.paused === undefined;
        if (stop) f.dataset.paused = ""; else delete f.dataset.paused;
        e.currentTarget.setAttribute("aria-label", stop ? "Play" : "Pause");
        $$("video", f).forEach(v => { if (stop) v.pause(); else if (v.src && v.offsetParent) v.play().catch(() => {}); });
      });
    });
  }

  /* ---------- ⌘K: every page, every section ---------- */
  const pal = $("#palette"), input = $("#palette input"), list = $("#palette .results");
  let index = null, hits = [], sel = 0;
  const esc = s => s.replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const score = (e, words) => {
    const t = e.t.toLowerCase(), b = e.b.toLowerCase(), p = e.p.toLowerCase();
    let s = 0;
    for (const w of words) {
      if (t.startsWith(w)) s += 8; else if (t.includes(w)) s += 5;
      else if (p.includes(w)) s += 2; else if (b.includes(w)) s += 1; else return 0;
    }
    return s + (e.d === 1 ? 1 : 0);
  };
  const show = () => {
    const q = input.value.trim().toLowerCase();
    const words = q.split(/\s+/).filter(Boolean);
    hits = !index ? [] : !words.length ? index.filter(e => e.d <= 1).slice(0, 12)
      : index.map(e => [score(e, words), e]).filter(x => x[0]).sort((a, b) => b[0] - a[0]).slice(0, 30).map(x => x[1]);
    sel = 0;
    list.innerHTML = hits.length ? hits.map((e, i) =>
      `<li><a href="${e.u}" role="option" aria-selected="${i === 0}"><span class="where">${esc(e.p)}</span><span class="what">${esc(e.t)}</span>${e.b ? `<span class="snip">${esc(e.b)}</span>` : ""}</a></li>`).join("")
      : `<li class="none">${window.SNYVI_MARK ? window.SNYVI_MARK("rest") : ""}${index ? "Nothing here matches that." : "Reading the pages…"}</li>`;
  };
  const move = d => {
    const as = $$("a", list);
    if (!as.length) return;
    sel = (sel + d + as.length) % as.length;
    as.forEach((a, i) => a.setAttribute("aria-selected", i === sel));
    as[sel].scrollIntoView({ block: "nearest" });
  };
  const openSearch = () => {
    if (!pal || pal.open) return;
    pal.showModal(); input.value = ""; show(); input.focus();
    if (!index) fetch("/search.json").then(r => r.json()).then(j => { index = j; show(); }).catch(() => {
      list.innerHTML = `<li class="none">The search index didn't load. Every page is in the sidebar.</li>`;
    });
  };
  input?.addEventListener("input", show);
  input?.addEventListener("keydown", e => {
    if (e.key === "ArrowDown") { e.preventDefault(); move(1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); move(-1); }
    else if (e.key === "Enter") { const a = $$("a", list)[sel]; if (a) { e.preventDefault(); pal.close(); location.href = a.href; } }
  });
  list?.addEventListener("click", e => { if (e.target.closest("a")) pal.close(); });
  pal?.addEventListener("click", e => { if (e.target === pal) pal.close(); });
  $$("[data-search]").forEach(b => b.addEventListener("click", openSearch));

  /* ---------- keys, as the window has them ---------- */
  document.addEventListener("keydown", e => {
    const typing = e.target.closest("input, textarea, [contenteditable]");
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); openSearch(); return; }
    if (e.key === "Escape") { if (menu && !menu.hidden) { openMenu(false); themeBtn.focus(); } toggleNav(false); return; }
    if (typing || e.metaKey || e.ctrlKey || e.altKey || document.querySelector("dialog[open]")) return;
    if (e.key === "/") { e.preventDefault(); openSearch(); }
    else if (e.key === "t") { e.preventDefault(); openMenu(menu.hidden); }
    else if (e.key === "f" && film) { e.preventDefault(); playFilm(); }
  });
  menu?.addEventListener("keydown", e => {
    if (!["ArrowDown", "ArrowUp"].includes(e.key)) return;
    e.preventDefault();
    const items = $$("[data-set]", menu), i = items.indexOf(document.activeElement);
    items[(i + (e.key === "ArrowDown" ? 1 : items.length - 1)) % items.length].focus();
  });

  /* ---------- where you are in the story ---------- */
  const heads = $$(".doc h2[id], .doc h3[id]");
  const links = id => $$(`.sub a[href$="#${CSS.escape(id)}"], .toc a[href="#${CSS.escape(id)}"]`);
  if (heads.length && "IntersectionObserver" in window) {
    let current = null;
    const seen = new Map();
    const pick = () => {
      // The last heading above the top third of the window is the one being read.
      const line = innerHeight * 0.33;
      let best = null;
      for (const h of heads) { if (h.getBoundingClientRect().top <= line) best = h; else break; }
      const id = best?.id || null;
      if (id === current) return;
      if (current) links(current).forEach(a => a.classList.remove("on"));
      if (id) links(id).forEach(a => a.classList.add("on"));
      current = id;
    };
    const io = new IntersectionObserver(es => { es.forEach(e => seen.set(e.target, e.isIntersecting)); pick(); }, { rootMargin: "0px 0px -60% 0px" });
    heads.forEach(h => io.observe(h));
    addEventListener("scroll", () => requestAnimationFrame(pick), { passive: true });
    pick();
  }

  /* ---------- diagrams: mermaid, only where there is one ---------- */
  function redrawDiagrams() {
    const pres = $$("pre.mermaid");
    if (!pres.length) return;
    mermaidP ||= new Promise((res, rej) => {
      const s = document.createElement("script");
      s.src = "/assets/mermaid.js"; s.onload = res; s.onerror = rej;
      document.head.appendChild(s);
    });
    mermaidP.then(() => {
      const cs = getComputedStyle(root), v = n => cs.getPropertyValue(n).trim();
      const dark = cs.colorScheme === "dark";
      window.mermaid.initialize({
        startOnLoad: false, securityLevel: "strict", theme: "base", fontFamily: "Inter, sans-serif",
        // Drawn at their own size and scrolled if wider than the page: shrunk to fit, a wide flow's labels went to 8 px.
        flowchart: { useMaxWidth: false }, sequence: { useMaxWidth: false }, state: { useMaxWidth: false },
        themeVariables: {
          darkMode: dark, background: v("--bg"), fontSize: "14px",
          primaryColor: v("--bg-raise"), primaryTextColor: v("--fg"), primaryBorderColor: v("--rule-2"),
          lineColor: v("--fg-3"), secondaryColor: v("--bg-side"), tertiaryColor: v("--code-bg"),
          clusterBkg: v("--bg-side"), clusterBorder: v("--rule-2"), edgeLabelBackground: v("--bg"),
          noteBkgColor: v("--accent-bg"), noteTextColor: v("--fg"), noteBorderColor: v("--accent"),
          actorBkg: v("--bg-raise"), actorBorder: v("--rule-2"), actorTextColor: v("--fg"), signalColor: v("--fg-2"), signalTextColor: v("--fg"),
        },
      });
      pres.forEach(p => {
        p.dataset.src ||= p.textContent;
        p.removeAttribute("data-processed");
        p.innerHTML = "";
        p.textContent = p.dataset.src;
      });
      return window.mermaid.run({ nodes: pres });
    }).then(() => pres.forEach(p => {
      p.dataset.done = "";
      // Wider than a phone: open on its middle, where the flow's heart is, not its left edge.
      p.scrollLeft = (p.scrollWidth - p.clientWidth) / 2;
    })).catch(() => pres.forEach(p => p.dataset.done = ""));
  }
  if ("requestIdleCallback" in window) requestIdleCallback(redrawDiagrams); else setTimeout(redrawDiagrams, 200);

  /* ---------- the arrival: once a visit ---------- */
  let seen = null;
  try { seen = sessionStorage.getItem("snyvi-site-arrived"); sessionStorage.setItem("snyvi-site-arrived", "1"); } catch {}
  if (root.dataset.arrive !== undefined && !seen) {
    root.classList.add("arrived");
    if (!reduced) {
      const marks = $$(".bar .mk, .side .brand .mk");
      marks.forEach(m => setFace(m, "whoa"));
      setTimeout(() => marks.forEach(m => setFace(m, "rest")), 2400);
    }
  }
})();
