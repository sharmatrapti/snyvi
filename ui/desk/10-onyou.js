/* ui/desk/10-onyou.js: a part of desk.js, one module. build.rs joins ui/desk/*.js in name
 * order (src/strip.rs `source`); SNYVI_UI_DIR serves the same join. */
// ---------- on you: what only the reader can do, in the rail's corner ----------
/*
 * Your turn was a block of cards at the top of the rail: fixed, in the
 * accent, and it came and went above the panels, so every question an agent
 * asked pushed the desk down and every answer pulled it back up. Half of
 * what it showed was Claude's own question dialog, already on screen in its
 * panel and answered there (audit e467fa057a).
 *
 * So it lives in a corner, floating, where nothing it does moves the page:
 * the rail's, at the rail's width, over the rail's own foot. The window's
 * bottom-right, where it was first, is the rightmost panel's, and the card
 * sat on Claude Code's prompt there (audit e0a854b732, U1). A turn new to
 * this page rises as a small card, with a soft chime; untouched for a few
 * seconds, or once the reader clicks elsewhere, it settles into the pill,
 * "On you 2", and the pill opens the tray: every fresh turn, newest first,
 * the older ones folded, and Suggested. Claude's own dialog never rises
 * (09-thread.js `shown`), and a turn that has gone quiet -- its panel
 * closed, half an hour old, or its panel prompted since -- never rises or
 * rings. With no rail in view (folded, or a narrow window) nothing rises
 * over a panel: the pill lights, in the window's corner.
 *
 * It is the one corner every page has (ui/corner.js, plan b0bca0a11f): the
 * look is corner.js's, and so are the cards that rise from outside -- a page
 * from another desk, an aside, a friend's word -- an agent's offer, which
 * is On you here too, and Lately under it in the tray. This file draws them
 * where the desk's corner is, and hands their clicks back.
 */

/** How long a risen card stays before it settles into the pill. */
const RISE_MS = 8000;
/** The corner's state: the tray open; its older turns unfolded; the turn
 *  risen as a card (its id, or its group's first id) and the one whose rise
 *  has been drawn; the hover that holds a card; the turns this page has
 *  seen, for this desk, so only a new one rises; the tray pinned open
 *  (#115); and where the focus was when the tray opened, to give it back. */
const oy = { open: false, older: false, rise: 0, drawnRise: 0, timer: 0, hover: false, seen: null, at: null, pin: false, pinAt: null, back: null, inRail: false, more: 0, rang: 0 };
/** Arrivals this close together are one: one chime, and the card says the
 *  rest as "+N more" (U7). Four hand-overs in a second rang twice and
 *  showed only the last. */
const BURST_MS = 1500;
let oyEl = null;
/** The turns seen on each desk this page has been on (#116). Leaving a desk
 *  used to forget them, so a turn asked while the reader was on another desk
 *  or reading a document came back already counted and never rose: it sat in
 *  the pill, unsaid. Kept per desk, only a desk's first read learns quietly. */
const oySeen = new Map();

/** A tray pinned open stays open on this desk while the reader works in its
 *  panels (#115): remembered in this browser, per desk, as a convenience. */
const PIN_KEY = "snyvi.onyou.pin";
function pinned(id) {
  try { return (JSON.parse(localStorage.getItem(PIN_KEY) || "[]") || []).includes(id); } catch { return false; }
}
function setPinned(id, on) {
  oy.pin = on;
  try {
    const all = (JSON.parse(localStorage.getItem(PIN_KEY) || "[]") || []).filter(x => x !== id);
    if (on) all.push(id);
    localStorage.setItem(PIN_KEY, JSON.stringify(all));
  } catch {}
}

/** The corner's element, made once a desk draws it, at the toast's level. */
function oyNode() {
  if (oyEl && oyEl.isConnected) return oyEl;
  oyEl = document.createElement("div");
  oyEl.id = "onyou";
  oyEl.className = "oy";
  oyEl.setAttribute("role", "region");
  oyEl.setAttribute("aria-label", "On you");
  oyEl.addEventListener("click", click);
  oyEl.addEventListener("mouseenter", () => { oy.hover = true; });
  oyEl.addEventListener("mouseleave", () => { oy.hover = false; });
  document.body.append(oyEl);
  return oyEl;
}

/** What the pill counts: fresh waiting turns, suggestions and an agent's
 *  offers, which are in amber; and the older ones, which are not. */
function oyCounts(d, f) {
  const waiting = f.turns.filter(w => shown(w) && !w.answered_at);
  const now = waiting.filter(w => fresh(d, w)).length;
  return { now: now + f.suggestions.length + (ctx.corner?.()?.offerCount() || 0), old: waiting.length - now };
}

/** The pin, in the tray's head (#115): pinned, the tray stays open while
 *  the reader works in the panels, and a click elsewhere leaves it. The pill
 *  or Esc in it closes it, and unpins. On the head's line, not over the
 *  scrollbar, nor on the risen card, whose corner is its ✕ (U3, U4). */
const pinBtn = () => `<button type="button" class="oy-pin${oy.pin ? " on" : ""}" data-a="oy-pin" aria-pressed="${oy.pin}" aria-label="${oy.pin ? "Unpin" : "Pin open"}" data-tip="${oy.pin ? "Unpin" : "Pin open"}" data-tip-sub="${oy.pin ? "It closes again on a click elsewhere" : "Stays open while you work in the panels"}">${ico("pin")}</button>`;

/** The risen card: the one turn, as the tray would draw it, or its Undo
 *  once it was put away. Nothing, once it is answered and has nothing left
 *  to send, or gone from the list. */
function riseHtml(d, f, esc) {
  const w = f.turns.find(x => x.id === oy.rise || x.ask_group === oy.rise);
  const gone = filedGone && (filedGone.k === "w" || filedGone.k === "g") && filedGone.id === oy.rise ? filedGone : null;
  const body = gone ? goneRow(gone.k, gone.id, gone.text, esc)
    : !w || !shown(w) || (w.answered_at && !sendable.has(w.id) && !w.ask_group) ? "" : turnCard(d, f, w, esc);
  if (!body) return "";
  const first = oy.drawnRise !== oy.rise;
  oy.drawnRise = oy.rise;
  // What came with it, in the same burst: in the tray, a click away.
  const more = oy.more && !gone ? `<button type="button" class="oy-more" data-a="oy-open">+${oy.more} more</button>` : "";
  return `<div class="oy-card${first ? " rise" : ""}"><ul class="dk-turn-list" data-part="desk.turns">${body}</ul>${more}</div>`;
}

/** The tray, where the pill was: under its head (On you, the pin, and
 *  its ✕), what is fresh, newest first, the older ones folded under a line
 *  of their own, and Suggested. With nothing fresh above them the older ones
 *  are the tray, drawn open (#118), and the head says Older: an ON YOU head
 *  over an OLDER head over one card was two titles for one thing. */
function trayHtml(d, f, esc, old) {
  const gone = filedGone && (filedGone.k === "w" || filedGone.k === "g") ? goneRow(filedGone.k, filedGone.id, filedGone.text, esc) : "";
  const rows = gone + turnRows(d, f, esc, false);
  const said = rows ? `<ul class="dk-turn-list" data-part="desk.turns">${rows}</ul>` : "";
  const why = `data-tip="Older" data-tip-sub="Its panel closed, asked over half an hour ago, or its panel was prompted since. It goes by itself after three days, and can come back"`;
  const olderOnly = !rows && old;
  const older = !old ? ""
    : !rows ? `<ul class="dk-turn-list">${turnRows(d, f, esc, true)}</ul>`
    : `<button type="button" class="oy-older" data-a="oy-older" aria-expanded="${oy.older}" ${why}>${old} older</button>` +
      (oy.older ? `<ul class="dk-turn-list">${turnRows(d, f, esc, true)}</ul>` : "");
  const K = ctx.corner?.(), offers = K ? K.offersHtml() : "";
  const all = said + older + (offers ? `<ul class="dk-turn-list cn-list">${offers}</ul>` : "") + sugRows(f, d);
  const title = olderOnly ? `<h3 class="oy-h" ${why}>On you · older</h3>`
    : `<h3 class="oy-h" data-tip="On you" data-tip-sub="What only you can do on this desk. Your answer goes with your next message to the panel that asked">On you</h3>`;
  const close = `<button type="button" class="oy-x" data-a="oy-open" aria-label="Close" data-tip="Close" data-tip-sub="Esc" data-key="ctrl+alt+y">${ico("x")}</button>`;
  const head = `<div class="oy-head">${title}<span class="oy-gap"></span>${pinBtn()}${close}</div>`;
  return `<div class="oy-tray" role="dialog" aria-label="On you and Lately" tabindex="-1">${head}<div class="oy-scroll">${all || `<p class="oy-none">Nothing is on you here.</p>`}${K ? K.trayHtml() : ""}</div></div>`;
}

/** Where the corner sits: in the rail's column, at its width, its foot just
 *  above the rail's own last lines (the panel's pid and age, `ctx.metaEl`),
 *  so it covers neither a panel nor that line (U1, U2). With no rail in
 *  view -- folded to its strip, hidden, or a sheet on a narrow window -- the
 *  window's corner, and nothing rises there (`arrived`). */
function oyPlace(el) {
  const r = ctx.rail, rr = r && r.offsetParent && r.getBoundingClientRect();
  oy.inRail = !!rr && rr.width >= 180 && getComputedStyle(r).position !== "fixed";
  el.classList.toggle("in-rail", oy.inRail);
  if (!oy.inRail) return;
  const m = ctx.metaEl?.getBoundingClientRect(), foot = m && m.height ? m.top : rr.bottom;
  el.style.setProperty("--oy-l", `${Math.round(rr.left + 8)}px`);
  el.style.setProperty("--oy-w", `${Math.round(rr.width - 16)}px`);
  el.style.setProperty("--oy-b", `${Math.round(innerHeight - foot + 6)}px`);
  el.style.setProperty("--oy-max", `${Math.max(160, Math.round(foot - rr.top - 40))}px`);
}
addEventListener("resize", () => { if (oyEl?.isConnected) oyPlace(oyEl); });

/** The corner, drawn after the rail: the tray or the risen card, and the
 *  pill under them. Not there at all while nothing is on the reader. A desk
 *  whose tray was pinned opens with it open. */
function onYou(d) {
  const f = filedNow(d), { esc } = ctx, n = oyCounts(d, f);
  if (oy.pinAt !== d.id) { oy.pinAt = d.id; oy.pin = pinned(d.id); oy.open ||= oy.pin; }
  const el = oyNode();
  oyPlace(el);
  // Every draw asks what is new, not only a read of the list: a turn read
  // while the desk was out of view -- Home, a document -- rises when it is
  // back in view (#116). Only once this desk's list is in, or its first read
  // would learn an empty list and every waiting turn would rise.
  if (filedAt === d.id) arrived(d);
  if (oy.rise && oy.open) { oy.rise = 0; oy.more = 0; }
  const card = oy.rise ? riseHtml(d, f, esc) : "";
  if (oy.rise && !card) { oy.rise = 0; oy.more = 0; clearTimeout(oy.timer); }
  const tray = oy.open ? trayHtml(d, f, esc, n.old) : "";
  // What came from outside rises here too, over a turn's card never.
  const K = ctx.corner?.(), news = K && !tray && !card ? K.cardHtml(true) : "", lit = !!K?.dot();
  const say = (n.now ? `On you <span class="n">${n.now}</span>` : n.old ? `${n.old} older` : lit || K?.keep() ? "Lately" : "On you") +
    (lit ? `<span class="cn-dot" aria-label="something new in Lately"></span>` : "");
  // The open tray is closed by its head's ✕: a pill under it as well said
  // "1 older" a second time, over the rail's last row (the notes' field).
  const pill = !tray && (card || news || n.now || n.old || lit || K?.keep())
    ? `<button type="button" class="oy-pill${n.now ? " on" : ""}" data-a="oy-open" aria-expanded="false" data-tip="On you" data-tip-sub="What only you can do on this desk, and what came lately" data-key="ctrl+alt+y">${say}</button>` : "";
  // A redraw replaces every button: the one the keyboard was on gets the
  // focus back, by what it does and to which row, or a press on a card
  // button dropped the hand to the page's body (W24).
  const was = el.contains(document.activeElement) && document.activeElement.dataset?.a ? keyOf(document.activeElement) : "";
  drawing = true;
  const drew = drawIn(el, news + card + tray + pill);
  drawing = false;
  if (drew && was && !el.contains(document.activeElement)) el.querySelector(was)?.focus({ preventScroll: true });
  // The corner's toasts sit above it while it shows, not on it (toast.js,
  // whose own foot is 20 px up from the window's).
  const top = el.offsetHeight ? el.getBoundingClientRect().top : innerHeight;
  document.documentElement.style.setProperty("--oy-h", el.offsetHeight ? `${Math.max(0, Math.round(innerHeight - top - 20 + 8))}px` : "0px");
  if (drew) threadFocus(el);
}

/** A button's selector by what it does and which row it is on. */
function keyOf(b) {
  const ds = b.dataset, esc = window.CSS.escape;
  return `[data-a="${esc(ds.a)}"]` + ["w", "s", "g", "v", "f"].filter(k => ds[k] != null).map(k => `[data-${k}="${esc(ds[k])}"]`).join("");
}

/** A read of this desk's turns: one new to this page, fresh and not
 *  Claude's own dialog, rises with a chime -- over the pill, whatever it
 *  says (#116). The first read of a desk on this page only learns what is
 *  there -- what was waiting before is the pill's to say; after that a turn
 *  asked while the reader was elsewhere rises when they come back. In an
 *  open tray it does not rise -- it is drawn there already -- but it still
 *  rings, since a pinned tray is open while the reader works. */
function arrived(d) {
  const waiting = filed.turns.filter(w => shown(w) && !w.answered_at);
  if (oy.at !== d.id || !oy.seen) {
    oy.at = d.id;
    oy.seen = oySeen.get(d.id);
    if (!oy.seen) { oySeen.set(d.id, oy.seen = new Set(waiting.map(w => w.id))); return; }
  }
  const fresh_ = waiting.filter(w => !oy.seen.has(w.id) && fresh(d, w));
  for (const w of waiting) oy.seen.add(w.id);
  if (!fresh_.length) return;
  const w = fresh_[fresh_.length - 1];
  // Said, for a reader of the screen, in the desk's own live region (U8).
  const n = slotOf(d, w.pane);
  announce(`${n ? `Panel ${n}` : "An agent"} ${w.kind === "decide" ? "asks" : "hands you"}: ${w.text}`);
  // One chime for a burst, and the card's "+N more" counts the rest (U7):
  // cards, so a card of four questions is one.
  const burst = Date.now() - oy.rang < BURST_MS;
  if (!burst) ring();
  oy.rang = Date.now();
  if (oy.open || !oy.inRail) return;
  const cards = new Set(fresh_.map(x => x.ask_group || x.id)).size;
  oy.more = burst && oy.rise ? oy.more + cards : cards - 1;
  oy.rise = w.ask_group || w.id;
  oy.drawnRise = 0;
  riseFor(RISE_MS);
}

/** The card settles into the pill after `ms`: held while the cursor is on
 *  it, or a field in it is open, and while the page is out of sight -- a
 *  turn that rose in a minimised window had settled before anyone saw it,
 *  so it waits for the page to show (`visibilitychange` below). */
function riseFor(ms) {
  clearTimeout(oy.timer);
  oy.timer = setTimeout(() => {
    if (document.hidden) return;
    if (oy.hover || (thField && oyEl?.querySelector(".th-in"))) return riseFor(1500);
    settle();
  }, ms);
}
document.addEventListener("visibilitychange", () => { if (!document.hidden && oy.rise) riseFor(RISE_MS); });
function settle() {
  clearTimeout(oy.timer);
  if (!oy.rise) return;
  oy.rise = 0; oy.more = 0;
  if (current()) rail();
}

/** The pill: the tray opens over it, or closes. */
function oyToggle() {
  if (oy.open) return oyClose(true);
  const at = document.activeElement;
  oy.back = at && !oyEl?.contains(at) ? at : null;
  oy.open = true; oy.rise = 0; clearTimeout(oy.timer);
  ctx.corner?.()?.opened();
  rail();
  oyEl?.querySelector(".oy-tray")?.focus({ preventScroll: true });
}
/** The tray asked for from outside the desk's own buttons: a desktop
 *  notification's click (`#onyou`), ⌘K. */
export function openTray() { if (!oy.open && current()) oyToggle(); }
/** The pin: on, the tray opens if it was a risen card, and stays; off, it
 *  stays open until a click elsewhere, as an unpinned tray does. */
function oyPin(d) {
  setPinned(d.id, !oy.pin);
  if (oy.pin && !oy.open) { oy.open = true; oy.rise = 0; clearTimeout(oy.timer); ctx.corner?.()?.opened(); }
  rail();
  oyEl?.querySelector(".oy-pin")?.focus({ preventScroll: true });
}
/** Closed: by Esc or the pill, the focus goes back where it was, and a pin
 *  comes off -- closing it is saying so; by a click elsewhere, it stays
 *  where that click put it. */
function oyClose(back) {
  if (back && oy.pin) setPinned(deskId, false);
  oy.open = false; oy.older = false;
  if (current()) rail();
  if (back && oy.back?.isConnected) oy.back.focus({ preventScroll: true });
  oy.back = null;
}

// A press anywhere else settles a risen card and closes the tray, unless it
// is pinned; Esc closes the tray before the desk's own keys see it -- a
// pinned one only from inside it, so Esc in a panel still reaches the panel.
document.addEventListener("pointerdown", e => {
  if (!oyEl?.isConnected || oyEl.contains(e.target)) return;
  if (oy.open) { if (!oy.pin) oyClose(false); }
  else if (oy.rise && !(thField && oyEl.querySelector(".th-in"))) settle();
}, true);
document.addEventListener("keydown", e => {
  // Ctrl+Alt+Y ("you") opens and closes the tray from anywhere on the desk,
  // a panel's keys included: before the panel's own handler, so nothing of
  // it is typed there (U8).
  if (e.code === "KeyY" && e.ctrlKey && e.altKey && !e.metaKey && !e.shiftKey && !altGr(e) && oyEl?.isConnected && current()) {
    e.preventDefault(); e.stopPropagation(); oyToggle(); return;
  }
  if (e.key !== "Escape" || !oy.open || !oyEl?.isConnected) return;
  if (oy.pin && !oyEl.contains(document.activeElement)) return;
  e.preventDefault(); e.stopPropagation();
  oyClose(true);
}, true);

/** Leaving the desk: the corner goes with it, to be the page's own again
 *  (ui/corner.js). What it has seen stays, in `oySeen`, and the pin, in
 *  this browser. */
function forgetOnYou() {
  clearTimeout(oy.timer);
  Object.assign(oy, { open: false, older: false, rise: 0, drawnRise: 0, hover: false, seen: null, at: null, pin: false, pinAt: null, back: null, more: 0 });
  oyEl?.remove(); oyEl = null;
  document.documentElement.style.setProperty("--oy-h", "0px");
}

/** A new turn's sound, and the taskbar's nudge in a window not in front:
 *  the corner's, one chime for every page (ui/corner.js `ring`). */
function ring() { ctx.corner?.()?.ring(); }

const ONYOU_CSS = `
/* In the rail's column (oyPlace): its width, its foot above the rail's own
   last lines; the card and the tray as wide as the column. */
.oy.in-rail { left: var(--oy-l); right: auto; bottom: var(--oy-b); width: var(--oy-w); align-items: stretch; }
.oy.in-rail > .oy-card, .oy.in-rail > .oy-tray { width: auto; }
.oy.in-rail > .oy-pill { align-self: flex-end; }
.oy.in-rail .oy-scroll { max-height: calc(var(--oy-max) - 40px); }
/* The card that rises: one turn, small. Its question is said whole (#114):
   cut at two lines, a key's steps ended in "then..." with no way to read on.
   A long one scrolls in its card rather than push the card off the screen. */
.oy-card > .dk-turn-list { margin: 0; }
.oy-card .tn-q { max-height: 9.6em; overflow-y: auto; overflow-wrap: anywhere; }
.oy.in-rail .oy-card .tn-q { max-height: 16em; }
.oy-more { display: block; width: 100%; padding: 3px 8px; border: 1px solid var(--rule); border-top: 0; border-radius: 0 0 var(--r-sm) var(--r-sm); background: var(--bg-raise); font-size: var(--fs-micro); color: var(--fg-2); text-align: left; }
.oy-more:hover { color: var(--fg); }
/* The tray's look is corner.js's: its head (On you, and the pin) stays;
   the rest scrolls under it. */
/* In the tray a turn is a row, ruled off from the next, not a card in the
   tray's box: a box in a box, at the rail's width, wrapped its answers onto
   two lines. The risen card, alone over the rail, keeps its card. */
.oy-tray .dk-turn-list { gap: 0; margin: 0; }
.oy-tray .dk-turn, .oy-tray .dk-sugcard { padding: 8px 0; border: 0; border-radius: 0; background: none; }
.oy-tray .dk-turn-list > li + li, .oy-tray .dk-turn-list + .dk-turn-list > li:first-child { border-top: 1px solid var(--rule); }
.oy-tray .dk-turn:not(.grp) > .tn-acts > .tn-x, .oy-tray .tn-gx { top: 6px; right: -4px; }
.oy-pin { display: grid; place-items: center; flex: none; width: 20px; height: 20px; border: 0; border-radius: var(--r-xs); background: none; color: var(--fg-3); cursor: pointer; }
.oy-pin:hover { background: var(--rule-2); color: var(--fg); }
.oy-pin.on { color: var(--accent); }
.oy-pin.on svg { fill: currentColor; }
.oy-older { display: block; margin: 8px 2px 0; font-size: var(--fs-micro); color: var(--fg-3); text-decoration: underline dotted; }
/* The pill floats over the rail's foot: the rail keeps that much room at
   its end, always, so its last row -- the notes' field -- scrolls clear of
   the pill rather than sit under it, and nothing moves as the pill comes. */
#toc:has(> .dk-rail) { padding-bottom: 40px; }
.oy .dk-note.gone { list-style: none; display: flex; align-items: center; gap: 6px; border: 1px dashed var(--rule-2); background: var(--bg-raise); }
`;
