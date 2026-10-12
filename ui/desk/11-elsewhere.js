/* ui/desk/11-elsewhere.js: a part of desk.js, one module. build.rs joins ui/desk/*.js in name
 * order (src/strip.rs `source`); SNYVI_UI_DIR serves the same join. */
// ---------- elsewhere: a document from another desk ----------
/*
 * A reader at work on one desk heard nothing when a panel on another desk
 * sent them something (#122): the waiting bar stays off on a desk (it lay
 * over the desk's head), the Desks list says only who is running and who
 * needs them, the rail lists this desk's own documents, and the desktop
 * notification is for a window that is not in front. It waited, unsaid,
 * in a sidebar section that is often folded.
 *
 * So it rises in this desk's corner, over Your turn, as a small card -- who
 * sent it, its title, Open and Later -- and settles after a few seconds, or
 * at a click elsewhere, the way a risen turn does. A burst is one card that
 * counts. Open reads it over this desk, so Back comes back here; Later, or
 * the card settling, leaves it where it was already waiting. No sound: the
 * reader chose the card alone. This desk's own documents are the rail's,
 * at the top of Documents, and do not rise -- but for its first: a reader
 * new to snyvi, working in the panel, never looked at the rail's edge, so
 * the first page a desk is sent rises too, once, and says where pages
 * live. The panel that sent it says so in its head as well (rail.js
 * `pageChips`), for every page, until it is read.
 */

/** The documents from elsewhere that rose and have not settled, newest
 *  first; the card is drawn once with its rise, and redrawn still after. */
const ew = { docs: [], first: null, timer: 0, drawn: false };

/** A document arrived (app.js `docEvent`) while this desk is on screen. One
 *  from this desk's own panels is the rail's to show; anything else -- a
 *  panel on another desk, a hook, the CLI -- rises. */
export function elsewhere(doc) {
  const d = current();
  if (!d || !doc) return;
  if (doc.desk && doc.desk.id === d.id) {
    // The desk's first: nothing listed and nothing removed before it. The
    // list is fetched after this (app.js `deskDocs`), so it is still the
    // list from before the page came.
    if (docsAt !== d.id || docList.length || docOff.length) return;
    ew.first = doc;
  } else ew.docs = [doc, ...ew.docs.filter(x => x.id !== doc.id)];
  ew.drawn = false;
  clearTimeout(ew.timer);
  ew.timer = setTimeout(function later() {
    if (document.hidden || oy.hover) { ew.timer = setTimeout(later, 1500); return; }
    ewSettle();
  }, RISE_MS);
  rail();
}

/** Who sent it, as the reader knows them: the desk and its panel, or the
 *  project it went to when no panel sent it. */
function ewFrom(doc, esc) {
  return doc.desk ? `${esc(doc.desk.name)} · panel ${doc.desk.slot}` : esc(doc.project);
}

/** The card, above Your turn's in the corner. */
function ewHtml(esc) {
  const [doc] = ew.docs;
  if (!doc) return ew.first ? firstHtml(ew.first, esc) : "";
  const n = ew.docs.length;
  const said = n > 1 ? `${n} documents from other desks` : `${ewFrom(doc, esc)} sent a document`;
  const first = !ew.drawn;
  ew.drawn = true;
  return `<div class="oy-card oy-doc${first ? " rise" : ""}" role="status"><p class="oy-from">${said}</p>` +
    `<p class="oy-title" data-tip="${esc(doc.title)}" data-tip-sub="${n > 1 ? "the newest" : ewFrom(doc, esc)}" data-tip-overflow>${esc(doc.title)}</p>` +
    `<div class="oy-acts"><button type="button" class="tn-opt rec" data-a="ew-open">Open</button><button type="button" class="tn-opt" data-a="ew-later" data-tip="Later" data-tip-sub="It waits in the sidebar">Later</button></div></div>`;
}

/** The desk's first page: who sent it, its title, and where pages stay. */
function firstHtml(doc, esc) {
  const first = !ew.drawn;
  ew.drawn = true;
  const who = doc.desk ? `Panel ${doc.desk.slot}` : "This desk";
  return `<div class="oy-card oy-doc${first ? " rise" : ""}" role="status"><p class="oy-from">${who} sent this desk's first page</p>` +
    `<p class="oy-title" data-tip="${esc(doc.title)}" data-tip-overflow>${esc(doc.title)}</p>` +
    `<div class="oy-acts"><button type="button" class="tn-opt rec" data-a="ew-open">Read it</button><button type="button" class="tn-opt" data-a="ew-later" data-tip="Later" data-tip-sub="It waits under Documents">Later</button></div>` +
    `<p class="oy-from oy-where">Every page stays under <b>Documents</b>, in the rail.</p></div>`;
}

/** Open: the newest, read over this desk. Later, a settle, or leaving the
 *  desk: the card goes, and what it said waits where it was. */
function ewAct(a) {
  if (a === "ew-open") { const doc = ew.docs[0] || ew.first; ewSettle(); if (doc) ctx.read(doc.id); return true; }
  if (a === "ew-later") { ewSettle(); return true; }
  return false;
}
function ewSettle() {
  clearTimeout(ew.timer);
  if (!ew.docs.length && !ew.first) return;
  ew.docs = [];
  ew.first = null;
  if (current()) rail();
}
document.addEventListener("pointerdown", e => {
  if ((ew.docs.length || ew.first) && oyEl?.isConnected && !oyEl.contains(e.target)) ewSettle();
}, true);

/** The page's foot while the panel that sent it waits on the reader: who,
 *  and the way back to it, where the page's last line (often a question)
 *  was just read. Drawn with the page and on each panel's change (app.js
 *  `turnLine`); once drawn its box stays, hidden, so the page's height
 *  never changes. */
export function turnLine(state, docEl) {
  const from = state.view === "doc" && state.doc && state.doc.desk;
  const d = from && from.slot && state.desks && state.desks.desks.find(x => x.id === from.id);
  const p = d && d.panes.find(x => x.slot === from.slot), s = p && p.status;
  let el = docEl.querySelector(".doc-turn");
  if (!(s && s.running && (s.blocked || s.agent === "done" || s.agent === "needs_you"))) { if (el) el.style.visibility = "hidden"; return; }
  if (!el) { el = document.createElement("aside"); el.className = "doc-turn"; docEl.append(el); }
  const who = /^claude\b/.test((s.cmd || p.cmd || "").trim()) ? "Claude is" : `Panel ${p.slot} is`;
  el.innerHTML = `<span>${who} waiting for you.</span><a href="/desk/${d.id}" data-desk="${d.id}" data-slot="${p.slot}" class="btn">Back to panel ${p.slot}</a>` +
    (state.deskBehind === d.id ? `<kbd>ctrl \u0060</kbd>` : "");
  el.style.visibility = "";
}

const ELSEWHERE_CSS = `
.oy-doc { box-sizing: border-box; padding: 10px 12px; border: 1px solid var(--rule); background: var(--bg-raise); }
.oy-from { margin: 0; font-size: var(--fs-micro); color: var(--fg-3); }
.oy-title { margin: 2px 0 8px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.oy-acts { display: flex; gap: 6px; }
.oy-where { margin-top: 8px; }
.oy-where b { font-weight: 600; color: var(--fg-2); }
.doc-turn { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin: 40px 0 0; padding: 12px 14px; border: 1px solid var(--rule-2); border-radius: var(--r-md); background: var(--bg-raise); font-size: var(--fs-ui); }
.doc-turn span { flex: 1; min-width: 0; color: var(--fg); font-weight: 500; }
`;
