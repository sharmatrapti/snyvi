/* ui/desk/11-arrivals.js: a part of desk.js, one module. build.rs joins ui/desk/*.js in name
 * order (src/strip.rs `source`); SNYVI_UI_DIR serves the same join. */
// ---------- arrivals: what this desk's panels sent, waiting ----------
/*
 * The queue bar (app.js 03-queue.js) is a reading page's. On a desk it lay
 * over the head -- the folder, Left off, and the window's drag grip -- with
 * every desk's documents in it, saying again what the rail's Documents says,
 * and with some fifty arrivals a day it was nearly always there (audit
 * dc5adb4641). So a desk has no bar: its head has a slot, beside the key,
 * with a document and this desk's count, and the sheet under it lists what
 * waits here, to open or to mark read. Other desks' wait in the sidebar.
 * The slot is there with nothing waiting, so a count arriving moves nothing;
 * at nought it shows only when the head is hovered, as Left off does.
 */

/** The desk whose sheet is open; Mark these read's Undo, { desk, ids, n,
 *  timer }; and the count last drawn, so one more lights the slot once. */
let arrOpen = null, arrGone = null, arrWas = -1;

/** What waits on this desk: its documents not yet read, newest first, as
 *  the rail lists them. Nothing until the rail's list is this desk's. */
const arrWaiting = d => docsAt === d.id ? docList.filter(x => x.unread) : [];

/** The slot in the head, drawn with the rail (06-rail.js `rail`). */
function arrSlot(d) {
  const hd = ctx.docEl.querySelector(".dk-head");
  if (!hd) return;
  let el = hd.querySelector(".dk-arr");
  if (!el) { el = Object.assign(document.createElement("span"), { className: "dk-arr" }); hd.querySelector(".dk-keys, .dk-tabs").before(el); }
  let b = el.querySelector(".dk-arr-b");
  if (!b) { b = Object.assign(document.createElement("button"), { type: "button", className: "dk-arr-b" }); b.dataset.a = "arrivals"; b.setAttribute("aria-haspopup", "dialog"); el.prepend(b); }
  // The head was drawn again under an open sheet: the sheet went with it.
  if (arrOpen === d.id && !el.querySelector(".dk-arr-sheet")) arrClose();
  const n = arrWaiting(d).length, open = arrOpen === d.id;
  b.classList.toggle("none", !n);
  b.setAttribute("aria-expanded", String(open));
  const html = head("doc") + (n ? `<span class="n">${n}</span>` : "");
  if (b.$html !== html) {
    b.innerHTML = html; b.$html = html;
    // One more lights it, once, in its background: nothing moves.
    if (arrWas >= 0 && n > arrWas) { b.classList.remove("kept"); void b.offsetWidth; b.classList.add("kept"); }
  }
  arrWas = n;
  b.setAttribute("aria-label", n ? `${n} waiting on this desk` : "Nothing waiting on this desk");
  b.dataset.tip = n ? `${ctx.plural(n, "document")} waiting on this desk` : "Nothing waiting on this desk";
  b.dataset.tipSub = n ? "sent by its panels · click to see them" : "other desks' documents wait in the sidebar";
  if (open) arrRows(d);
}

/** The sheet under the slot, or closed again. */
function arrSheet(d) {
  if (arrOpen === d.id) { arrClose(true); return; }
  arrClose(); keysClose();
  const slot = ctx.docEl.querySelector(".dk-arr");
  if (!slot) return;
  arrOpen = d.id;
  const sheet = Object.assign(document.createElement("div"), { className: "dk-keys-sheet dk-arr-sheet", tabIndex: -1 });
  sheet.setAttribute("role", "dialog"); sheet.setAttribute("aria-label", "Waiting on this desk");
  // Opening one reads it over the desk (07-actions.js `click`): the sheet
  // closes first, so it is not left open behind the document.
  sheet.addEventListener("click", e => { if (e.target.closest("a[data-read]")) arrClose(); });
  slot.append(sheet);
  arrSlot(d);
  document.addEventListener("pointerdown", arrOutside, true);
  document.addEventListener("keydown", arrKey, true);
  sheet.focus({ preventScroll: true });
}

/** The sheet's rows: each waiting document, opened by a click, and Mark
 *  these read for all of them, with its Undo where the button was. */
function arrRows(d) {
  const sheet = ctx.docEl.querySelector(".dk-arr-sheet");
  if (!sheet) return;
  const { esc } = ctx, w = arrWaiting(d), vs = d.panes.map(p => views.get(p.id)).filter(Boolean);
  const g = arrGone && arrGone.desk === d.id ? arrGone : null;
  const rows = w.map(x => `<li><a href="/d/${x.id}" data-read="${x.id}" data-tip="${esc(x.title)}" data-tip-sub="from ${esc(sentBy(vs, x.slot))} · ${ctx.fmt(x.received_at)}">${ico("doc")}<span class="title">${esc(x.title)}</span><span class="age">${ctx.relShort(x.received_at)}</span></a></li>`).join("");
  const html = `<div class="dk-keys-h">Waiting on this desk</div>` +
    (rows ? `<ul class="dk-arr-l">${rows}</ul>` : "") +
    (g ? `<div class="dk-keys-w" role="status"><span>Marked ${g.n} read</span><button type="button" class="dk-undo" data-a="arr-back">Undo</button></div>`
      : rows ? `<div class="dk-keys-w"><button type="button" data-a="arr-read">Mark ${w.length === 1 ? "it" : "these"} read</button></div>`
      : `<p class="dk-keys-none">Nothing waiting. What this desk's panels sent is in the rail's Documents; other desks' wait in the sidebar.</p>`);
  if (sheet.$html !== html) { sheet.innerHTML = html; sheet.$html = html; }
}

/** Mark these read: this desk's, and no other's. The Undo stands in the
 *  sheet for BACK_MS, as a removed note's does. */
async function arrRead(d) {
  const ids = arrWaiting(d).map(x => x.id);
  if (!ids.length) return;
  let j;
  try { j = await ctx.api("/api/queue/clear", { ids }); }
  catch (e) { ctx.toast("Could not mark them read", e); return; }
  if (!j.n) return;
  clearTimeout(arrGone?.timer);
  const g = arrGone = { desk: d.id, ids: j.ids, n: j.n };
  g.timer = setTimeout(() => { if (arrGone === g) { arrGone = null; const c = current(); if (c) arrSlot(c); } }, BACK_MS);
  // The rows go when the page hears "read" and the rail's list comes back;
  // the Undo is drawn now.
  for (const x of docList) if (g.ids.includes(x.id)) x.unread = false;
  arrSlot(d);
}
async function arrBack(d) {
  const g = arrGone;
  if (!g || g.desk !== d.id) return;
  clearTimeout(g.timer); arrGone = null;
  try { await ctx.api("/api/queue/unread", { ids: g.ids }); }
  catch (e) { ctx.toast("Could not bring them back", e); }
  arrSlot(d);
}

function arrOutside(e) { if (!e.target.closest(".dk-arr")) arrClose(); }
function arrKey(e) { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); arrClose(true); } }
function arrClose(back = false) {
  if (arrOpen == null) return;
  arrOpen = null;
  document.removeEventListener("pointerdown", arrOutside, true);
  document.removeEventListener("keydown", arrKey, true);
  ctx.docEl.querySelector(".dk-arr-sheet")?.remove();
  const b = ctx.docEl.querySelector(".dk-arr-b");
  if (b) { b.setAttribute("aria-expanded", "false"); if (back) b.focus(); }
}
/** Leaving the desk: the sheet, its Undo and the count go with it. */
function forgetArrivals() { arrClose(); clearTimeout(arrGone?.timer); arrGone = null; arrWas = -1; }

/** The slot's buttons: whether `a` was one of them (`act` asks). */
function arrAct(a, d) {
  if (a === "arrivals") arrSheet(d);
  else if (a === "arr-read") arrRead(d);
  else if (a === "arr-back") arrBack(d);
  else return false;
  return true;
}

const ARRIVALS_CSS = `
.dk-arr { position: relative; flex: none; display: flex; align-items: center; }
.dk-arr-b.none { opacity: 0; }
.dk-head:is(:hover, :focus-within) .dk-arr-b.none, .dk-arr-b.none[aria-expanded="true"] { opacity: 1; }
.dk-arr-b .n { color: var(--accent); font-weight: 600; }
.dk-arr-sheet { width: min(380px, calc(100vw - 32px)); }
.dk-arr-l { list-style: none; margin: 0; padding: 4px 0; max-height: 50vh; overflow-y: auto; }
.dk-arr-l a { display: flex; align-items: baseline; gap: 6px; padding: 4px 6px; border-radius: var(--r-sm); color: var(--fg); text-decoration: none; }
.dk-arr-l a:hover, .dk-arr-l a:focus-visible { background: var(--rule); }
.dk-arr-l svg { flex: none; align-self: flex-start; margin-top: 2px; color: var(--accent); }
.dk-arr-l .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dk-arr-l .age { flex: none; font-family: var(--mono); font-size: var(--fs-micro); color: var(--fg-3); font-variant-numeric: tabular-nums; }
.dk-arr-sheet .dk-keys-w { justify-content: flex-end; padding-top: 6px; border-top: 1px solid var(--rule); }
.dk-arr-sheet .dk-keys-w > span { margin-right: auto; }
.dk-arr-sheet .dk-keys-w .dk-undo { margin-left: 0; }
`;
