/* A gallery's viewer (src/widget/gallery.rs): what an agent's models made,
 * whole, over the desk. Opened at the tile that was clicked; ‹ › and ← →
 * step through the box's outputs, newest first. The switch beside them
 * shows every desk's galleries instead of this one's, and is remembered in
 * this browser. Pick, with a line, goes to the panel that added the
 * output, which hears it on its next prompt.
 *
 * It wears the note pictures' lightbox (`.dk-lb`, desk.js), which a desk
 * page has: a gallery is only ever in a desk's rail. A clip plays here and
 * nowhere else, so the window holds one player at a time. */

let open = null;
const ALL = "snyvi.galleryAll";
const allOn = () => { try { return localStorage.getItem(ALL) === "1"; } catch { return false; } };
const setAll = on => { try { localStorage.setItem(ALL, on ? "1" : "0"); } catch {} };

const STYLE = `
.gal-lb figure { width: min(1400px, calc(100vw - 64px)); align-items: center; }
.gal-lb .gal-m { display: grid; place-items: center; min-height: 0; max-height: calc(100vh - 170px); }
.gal-lb .gal-m > :is(img, video) { display: block; width: auto; height: auto; max-width: 100%; max-height: calc(100vh - 170px); object-fit: contain; border-radius: var(--r-sm); background: var(--bg-raise); box-shadow: var(--shadow); }
.gal-lb .gal-none { padding: 48px 64px; border-radius: var(--r-sm); background: var(--bg-raise); color: var(--fg-2); box-shadow: var(--shadow); }
.gal-lb figcaption { flex-wrap: wrap; width: 100%; box-sizing: border-box; }
.gal-lb figcaption .t { flex: 1 1 200px; min-width: 0; color: var(--fg); overflow-wrap: anywhere; }
.gal-lb .k { font-family: var(--mono); font-size: 11px; font-variant-numeric: tabular-nums; color: var(--fg-3); }
.gal-lb details { flex: 1 1 100%; max-height: 7em; overflow: auto; color: var(--fg-2); white-space: pre-wrap; }
.gal-lb .gal-pick { display: flex; gap: 6px; flex: 1 1 100%; }
.gal-lb .gal-pick input { flex: 1; min-width: 0; padding: 3px 8px; border: 1px solid var(--rule); border-radius: var(--r-xs); background: var(--bg); color: var(--fg); font: inherit; }
.gal-lb [aria-pressed="true"] { color: var(--accent); }
`;

const money = m => !m ? "" : m < 10000 ? "<$0.01" : `$${(m / 1e6).toFixed(m < 1e8 ? 2 : 0)}`;

/** Open the viewer at the tile `b`, in its box. */
export async function show(ctx, b) {
  const sec = b.closest("section.wg");
  if (!sec) return;
  if (!document.getElementById("gal-style")) {
    const s = document.createElement("style");
    s.id = "gal-style"; s.textContent = STYLE;
    document.head.append(s);
  }
  close();
  const el = document.createElement("div");
  el.className = "dk-lb gal-lb"; el.setAttribute("role", "dialog"); el.setAttribute("aria-modal", "true"); el.setAttribute("aria-label", "A gallery");
  open = { el, ctx, from: b, desk: +sec.dataset.wdesk, name: sec.dataset.w, all: allOn(), items: [], desks: {}, i: 0, at: +b.dataset.gal, picked: new Set() };
  el.addEventListener("click", onClick);
  el.addEventListener("keydown", onKey);
  document.body.append(el);
  await load();
}

async function load() {
  const o = open;
  if (!o) return;
  try {
    const j = await o.ctx.api(`/api/gallery?desk=${o.desk}&name=${encodeURIComponent(o.name)}${o.all ? "&all=1" : ""}`);
    if (o !== open) return;
    o.items = j.items || []; o.desks = j.desks || {};
    const i = o.items.findIndex(x => x.id === o.at);
    o.i = i >= 0 ? i : 0;
  } catch {
    o.items = []; o.err = true;
  }
  draw();
}

function draw() {
  const o = open;
  if (!o) return;
  const esc = o.ctx.esc, x = o.items[o.i], n = o.items.length;
  // Where the reader was typing survives a step: it is the same pick.
  const typed = o.el.querySelector(".gal-pick input")?.value || "";
  let media;
  if (!x) media = `<p class="gal-none">${o.err ? "Couldn't read the gallery. Close it, and click a tile again." : "Nothing here yet."}</p>`;
  else if (x.failed) media = `<p class="gal-none">It failed: ${esc(x.failed)}</p>`;
  else if (!x.file) media = `<p class="gal-none">Being made${x.model ? ` · ${esc(x.model)}` : ""}…</p>`;
  else if (x.kind === "video") media = `<video controls preload="metadata" src="/gallery/${esc(x.file)}"${x.poster ? ` poster="/gallery/${esc(x.poster)}"` : ""}></video>`;
  else media = `<img src="/gallery/${esc(x.file)}" alt="${esc(x.caption || x.job || "")}"${x.width ? ` width="${x.width}" height="${x.height}"` : ""}>`;
  const what = x ? esc(x.caption || x.job || x.source.split(/[\\/]/).pop() || "An output") : "";
  const meta = x ? [x.model && esc(x.model), money(x.cost_micros), o.all && esc(o.desks[x.desk_id] || `desk ${x.desk_id}`), x.writer && esc(x.writer)].filter(Boolean).join(" · ") : "";
  const pickable = x && x.file && !x.failed;
  o.el.innerHTML = `<figure><div class="gal-m">${media}</div><figcaption>` +
    `<span class="t">${what}${meta ? ` <span class="k">${meta}</span>` : ""}</span>` +
    (n > 1 ? `<span class="k">${o.i + 1} of ${n}</span><button type="button" data-g="prev" aria-label="The newer one">‹</button><button type="button" data-g="next" aria-label="The older one">›</button>` : "") +
    `<button type="button" data-g="all" aria-pressed="${o.all}" data-tip="${o.all ? "Showing every desk's galleries" : "Showing this desk's gallery"}">All desks</button>` +
    `<button type="button" data-g="close">Close</button>` +
    (x && x.prompt ? `<details><summary>Prompt</summary>${esc(x.prompt)}</details>` : "") +
    (pickable ? `<form class="gal-pick" data-g="form"><input name="line" maxlength="300" placeholder="What you like, or what to change (optional)" aria-label="A line for the agent" value="${esc(typed)}">` +
      `<button type="submit"${o.picked.has(x.id) ? " disabled" : ""}>${o.picked.has(x.id) ? "Picked · told on its next prompt" : "Pick"}</button></form>` : "") +
    `</figcaption></figure>`;
  const f = o.el.querySelector("form");
  if (f) f.addEventListener("submit", pick);
  (o.el.querySelector("[data-g=close]")).focus({ preventScroll: true });
}

async function pick(e) {
  e.preventDefault();
  const o = open, x = o?.items[o.i];
  if (!x) return;
  const btn = e.target.querySelector("button"), line = e.target.line.value.trim();
  btn.disabled = true;
  try {
    await o.ctx.api("/api/gallery/pick", { item: x.id, line });
    o.picked.add(x.id);
    btn.textContent = "Picked · told on its next prompt";
  } catch (err) {
    btn.disabled = false; btn.textContent = "Couldn't · again";
    btn.dataset.tip = err?.message || "";
  }
}

function onClick(e) {
  const o = open;
  if (!o) return;
  const b = e.target.closest("[data-g]");
  if (!b) { if (e.target === o.el) close(); return; }
  const k = b.dataset.g, n = o.items.length;
  if (k === "close") close();
  else if (k === "all") { o.all = !o.all; setAll(o.all); o.at = o.items[o.i]?.id; load(); }
  else if ((k === "prev" || k === "next") && n) { o.i = (o.i + (k === "next" ? 1 : n - 1)) % n; draw(); }
}

function onKey(e) {
  // The desk gives keys to the focused panel; these are the viewer's.
  e.stopPropagation();
  if (e.key === "Escape") { e.preventDefault(); close(); return; }
  if (e.target.closest?.("input")) return;
  if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
    e.preventDefault();
    open?.el.querySelector(`[data-g=${e.key === "ArrowRight" ? "next" : "prev"}]`)?.click();
  }
}

export function close() {
  if (!open) return;
  const { el, from } = open;
  open = null;
  el.remove();
  if (from?.isConnected) from.focus({ preventScroll: true });
}
