/* ui/desk/10-onyou.js: a part of desk.js, one module. build.rs joins ui/desk/*.js in name
 * order (src/strip.rs `source`); SNYVI_UI_DIR serves the same join. */
// ---------- on you: Your turn in the corner ----------
/*
 * Your turn was a block of cards at the top of the rail: fixed, in the
 * accent, and it came and went above the panels, so every question an agent
 * asked pushed the desk down and every answer pulled it back up. Half of
 * what it showed was Claude's own question dialog, already on screen in its
 * panel and answered there (audit e467fa057a).
 *
 * So it lives in the bottom-right corner, floating, where nothing it does
 * moves the page. A turn new to this page rises there as a small card, with
 * a soft chime; untouched for a few seconds, or once the reader clicks
 * elsewhere, it settles into the pill, "On you 2", and the pill opens the
 * tray: every fresh turn, the older ones folded, and Suggested. Claude's own
 * dialog never rises (09-thread.js `shown`), and a turn that has gone quiet
 * -- its panel closed, or half an hour old -- never rises or rings. Home
 * lists all of them, as it did.
 */

/** How long a risen card stays before it settles into the pill. */
const RISE_MS = 8000;
/** The corner's state: the tray open; its older turns unfolded; the turn
 *  risen as a card (its id, or its group's first id) and the one whose rise
 *  has been drawn; the hover that holds a card; the turns this page has
 *  seen, for this desk, so only a new one rises; and where the focus was
 *  when the tray opened, to give it back. */
const oy = { open: false, older: false, rise: 0, drawnRise: 0, timer: 0, hover: false, seen: null, at: null, back: null };
let oyEl = null;

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

/** What the pill counts: fresh waiting turns and suggestions, which are in
 *  the accent; and the older ones, which are not. */
function oyCounts(d, f) {
  const waiting = f.turns.filter(w => shown(w) && !w.answered_at);
  const now = waiting.filter(w => fresh(d, w)).length;
  return { now: now + f.suggestions.length, old: waiting.length - now };
}

/** The risen card: the one turn, as the tray would draw it, or its Undo
 *  once it was put away. Nothing, once it is answered and has nothing left
 *  to send, or gone from the list. */
function riseHtml(d, f, esc) {
  const w = f.turns.find(x => x.id === oy.rise);
  const gone = filedGone && filedGone.k === "w" && filedGone.id === oy.rise ? filedGone : null;
  const body = gone ? goneRow("w", gone.id, gone.text, esc)
    : !w || !shown(w) || (w.answered_at && !sendable.has(w.id) && !w.ask_group) ? "" : turnCard(d, f, w, esc);
  if (!body) return "";
  const first = oy.drawnRise !== oy.rise;
  oy.drawnRise = oy.rise;
  return `<ul class="dk-turn-list oy-card${first ? " rise" : ""}" data-part="desk.turns">${body}</ul>`;
}

/** The tray, over the pill: Your turn in the order asked, the older ones
 *  folded under a line of their own, and Suggested. */
function trayHtml(d, f, esc, old) {
  const gone = filedGone && filedGone.k === "w" ? goneRow("w", filedGone.id, filedGone.text, esc) : "";
  const rows = turnRows(d, f, esc, false) + gone;
  const said = rows ? `<h3 class="oy-h" data-tip="Your turn" data-tip-sub="What only you can do. Your answer goes with your next message to the panel that asked">Your turn</h3><ul class="dk-turn-list" data-part="desk.turns">${rows}</ul>` : "";
  const older = old ? `<button type="button" class="oy-older" data-a="oy-older" aria-expanded="${oy.older}" data-tip="Older" data-tip-sub="Its panel closed, or asked over half an hour ago. Home lists them too">${old} older</button>` +
    (oy.older ? `<ul class="dk-turn-list">${turnRows(d, f, esc, true)}</ul>` : "") : "";
  const all = said + older + sugRows(f, d);
  return `<div class="oy-tray" role="dialog" aria-label="On you" tabindex="-1">${all || `<p class="oy-none">Nothing is on you here.</p>`}</div>`;
}

/** The corner, drawn after the rail: the tray or the risen card, and the
 *  pill under them. Not there at all while nothing is on the reader. */
function onYou(d) {
  const f = filedNow(d), { esc } = ctx, n = oyCounts(d, f);
  if (oy.rise && oy.open) oy.rise = 0;
  const card = oy.rise ? riseHtml(d, f, esc) : "";
  if (oy.rise && !card) { oy.rise = 0; clearTimeout(oy.timer); }
  const tray = oy.open ? trayHtml(d, f, esc, n.old) : "";
  const say = n.now ? `On you <span class="n">${n.now}</span>` : n.old ? `${n.old} older` : "On you";
  const pill = card || tray || n.now || n.old
    ? `<button type="button" class="oy-pill${n.now ? " on" : ""}" data-a="oy-open" aria-expanded="${oy.open}" data-tip="${oy.open ? "Close" : "On you"}" data-tip-sub="${oy.open ? "Esc" : "What only you can do on this desk"}">${say}</button>` : "";
  const el = oyNode();
  drawing = true;
  const drew = drawIn(el, card + tray + pill);
  drawing = false;
  // The corner's toasts sit above it while it shows, not on it (toast.js).
  document.documentElement.style.setProperty("--oy-h", el.offsetHeight ? `${el.offsetHeight + 8}px` : "0px");
  if (drew) threadFocus(el);
}

/** A read of this desk's turns: one new to this page, fresh and not
 *  Claude's own dialog, rises with a chime. The first read of a desk only
 *  learns what is there -- what was waiting before is the pill's to say. */
function arrived(d) {
  const waiting = filed.turns.filter(w => shown(w) && !w.answered_at);
  if (oy.at !== d.id || !oy.seen) { oy.at = d.id; oy.seen = new Set(waiting.map(w => w.id)); return; }
  const fresh_ = waiting.filter(w => !oy.seen.has(w.id) && fresh(d, w));
  for (const w of waiting) oy.seen.add(w.id);
  if (!fresh_.length || oy.open) return;
  const w = fresh_[fresh_.length - 1];
  oy.rise = w.ask_group || w.id;
  oy.drawnRise = 0;
  chime();
  riseFor(RISE_MS);
}

/** The card settles into the pill after `ms`: held while the cursor is on
 *  it, or a field in it is open. */
function riseFor(ms) {
  clearTimeout(oy.timer);
  oy.timer = setTimeout(() => {
    if (oy.hover || (thField && oyEl?.querySelector(".th-in"))) return riseFor(1500);
    settle();
  }, ms);
}
function settle() {
  clearTimeout(oy.timer);
  if (!oy.rise) return;
  oy.rise = 0;
  if (current()) rail();
}

/** The pill: the tray opens over it, or closes. */
function oyToggle() {
  if (oy.open) return oyClose(true);
  const at = document.activeElement;
  oy.back = at && !oyEl?.contains(at) ? at : null;
  oy.open = true; oy.rise = 0; clearTimeout(oy.timer);
  rail();
  oyEl?.querySelector(".oy-tray")?.focus({ preventScroll: true });
}
/** Closed: by Esc or the pill, the focus goes back where it was; by a click
 *  elsewhere, it stays where that click put it. */
function oyClose(back) {
  oy.open = false; oy.older = false;
  if (current()) rail();
  if (back && oy.back?.isConnected) oy.back.focus({ preventScroll: true });
  oy.back = null;
}

// A press anywhere else settles a risen card and closes the tray; Esc
// closes the tray before the desk's own keys see it.
document.addEventListener("pointerdown", e => {
  if (!oyEl?.isConnected || oyEl.contains(e.target)) return;
  if (oy.open) oyClose(false);
  else if (oy.rise && !(thField && oyEl.querySelector(".th-in"))) settle();
}, true);
document.addEventListener("keydown", e => {
  if (e.key !== "Escape" || !oy.open || !oyEl?.isConnected) return;
  e.preventDefault(); e.stopPropagation();
  oyClose(true);
}, true);

/** Leaving the desk: the corner goes with it. */
function forgetOnYou() {
  clearTimeout(oy.timer);
  Object.assign(oy, { open: false, older: false, rise: 0, drawnRise: 0, hover: false, seen: null, at: null, back: null });
  oyEl?.remove(); oyEl = null;
  document.documentElement.style.setProperty("--oy-h", "0px");
}

/** A soft two-note chime, made here rather than loaded: quiet, short, and
 *  never in quiet mode. A page may sound only after the reader has clicked
 *  in it; before that the card still rises, silently. */
let audio = null;
function chime() {
  if (document.documentElement.dataset.mascot === "quiet") return;
  try {
    audio ||= new AudioContext();
    if (audio.state === "suspended") audio.resume().catch(() => {});
    const t = audio.currentTime;
    for (const [hz, at] of [[880, 0], [1318.5, 0.1]]) {
      const o = audio.createOscillator(), g = audio.createGain();
      o.type = "sine"; o.frequency.value = hz;
      g.gain.setValueAtTime(0.0001, t + at);
      g.gain.exponentialRampToValueAtTime(0.05, t + at + 0.012);
      g.gain.exponentialRampToValueAtTime(0.0001, t + at + 0.22);
      o.connect(g).connect(audio.destination);
      o.start(t + at); o.stop(t + at + 0.24);
    }
  } catch {}
}

const ONYOU_CSS = `
.oy { position: fixed; right: 20px; bottom: 20px; z-index: var(--z-toast); display: flex; flex-direction: column; align-items: flex-end; gap: 6px; pointer-events: none; }
.oy > * { pointer-events: auto; }
.oy-pill { padding: 3px 10px; border: 1px solid var(--rule-2); border-radius: var(--r-pill); background: var(--bg-raise); box-shadow: var(--shadow); font-size: var(--fs-micro); color: var(--fg-2); }
.oy-pill.on { border-color: var(--accent); color: var(--fg); }
.oy-pill .n { font-weight: 600; color: var(--accent); }
.oy-pill:hover, .oy-pill[aria-expanded="true"] { color: var(--fg); border-color: var(--accent); }
/* The card that rises: one turn, small, the question kept to two lines. */
.oy-card { width: 300px; margin: 0; box-shadow: var(--shadow); border-radius: var(--r-sm); }
.oy-card.rise { animation: oy-in var(--dur-move) var(--ease-out); }
.oy-card .tn-q { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
@keyframes oy-in { from { opacity: 0; transform: translateY(12px) scale(.96); } to { opacity: 1; transform: none; } }
.oy-tray { width: 320px; max-height: 60vh; overflow-y: auto; box-sizing: border-box; padding: 8px; border: 1px solid var(--rule); border-radius: var(--r-md); background: var(--bg); box-shadow: var(--shadow); animation: oy-in var(--dur-move) var(--ease-out); }
.oy-h { margin: 2px 2px 0; font-size: var(--fs-micro); font-weight: 600; color: var(--fg-3); text-transform: uppercase; letter-spacing: .04em; }
.oy-h ~ .oy-h { margin-top: 10px; }
.oy-older { display: block; margin: 8px 2px 0; font-size: var(--fs-micro); color: var(--fg-3); text-decoration: underline dotted; }
.oy-none { margin: 4px; font-size: var(--fs-small); color: var(--fg-3); }
.oy .dk-note.gone { list-style: none; display: flex; align-items: center; gap: 6px; border: 1px dashed var(--rule-2); background: var(--bg-raise); }
@media (prefers-reduced-motion: reduce) { .oy-card.rise, .oy-tray { animation: none; } }
`;
