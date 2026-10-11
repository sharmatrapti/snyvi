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
 * at the top of Documents, and do not rise.
 */

/** The documents from elsewhere that rose and have not settled, newest
 *  first; the card is drawn once with its rise, and redrawn still after. */
const ew = { docs: [], timer: 0, drawn: false };

/** A document arrived (app.js `docEvent`) while this desk is on screen. One
 *  from this desk's own panels is the rail's to show; anything else -- a
 *  panel on another desk, a hook, the CLI -- rises. */
export function elsewhere(doc) {
  const d = current();
  if (!d || !doc || (doc.desk && doc.desk.id === d.id)) return;
  ew.docs = [doc, ...ew.docs.filter(x => x.id !== doc.id)];
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
  if (!doc) return "";
  const n = ew.docs.length;
  const said = n > 1 ? `${n} documents from other desks` : `${ewFrom(doc, esc)} sent a document`;
  const first = !ew.drawn;
  ew.drawn = true;
  return `<div class="oy-card oy-doc${first ? " rise" : ""}" role="status"><p class="oy-from">${said}</p>` +
    `<p class="oy-title" data-tip="${esc(doc.title)}" data-tip-sub="${n > 1 ? "the newest" : ewFrom(doc, esc)}" data-tip-overflow>${esc(doc.title)}</p>` +
    `<div class="oy-acts"><button type="button" class="tn-opt rec" data-a="ew-open">Open</button><button type="button" class="tn-opt" data-a="ew-later" data-tip="Later" data-tip-sub="It waits in the sidebar">Later</button></div></div>`;
}

/** Open: the newest, read over this desk. Later, a settle, or leaving the
 *  desk: the card goes, and what it said waits where it was. */
function ewAct(a) {
  if (a === "ew-open") { const [doc] = ew.docs; ewSettle(); if (doc) ctx.read(doc.id); return true; }
  if (a === "ew-later") { ewSettle(); return true; }
  return false;
}
function ewSettle() {
  clearTimeout(ew.timer);
  if (!ew.docs.length) return;
  ew.docs = [];
  if (current()) rail();
}
document.addEventListener("pointerdown", e => {
  if (ew.docs.length && oyEl?.isConnected && !oyEl.contains(e.target)) ewSettle();
}, true);

const ELSEWHERE_CSS = `
.oy-doc { box-sizing: border-box; padding: 10px 12px; border: 1px solid var(--rule); background: var(--bg-raise); }
.oy-from { margin: 0; font-size: var(--fs-micro); color: var(--fg-3); }
.oy-title { margin: 2px 0 8px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.oy-acts { display: flex; gap: 6px; }
`;
