/* ui/desk/15-cards.js: a part of desk.js, one module. build.rs joins ui/desk/*.js in name
 * order (src/strip.rs `source`); SNYVI_UI_DIR serves the same join. */
// ---------- the cards above the rail's foot (#162) ----------
/*
 * The focused panel's passing word: a link to where its work can be seen (a
 * preview, a deploy, its PR) or one line on where it is ("Building 3/7"),
 * each for as long as it says. An agent puts them there with show_card
 * (`crate::card`, they come in the pane's status); the PR card snyvi works
 * out for itself from the panel's thread, so an agent never has to.
 *
 * Where they live is the point. The rail's foot is a fixed band (#163), and
 * the cards stand on it, over the bottom of the list, rather than in the
 * list's flow: a card coming or going moves no row of the rail. The list
 * always ends in room for three of them, so its last line can be scrolled
 * clear. They rise from *behind* the foot -- it is opaque and in front --
 * on a spring, and sink back behind it when their time is up, the
 * panel's focus moves, or the agent takes them off. The newest is nearest
 * the foot; three show, and the third counts the rest.
 *
 * The spring is the one motion here that is not the page's quiet ease: the
 * maker asked for cards that land like something with weight. It is
 * transform and opacity only, on the compositor, so the panels' canvases
 * are never repainted for it, and nothing runs once a card has landed.
 */

const CARD_H = 30, CARD_GAP = 6, CARD_LIFT = 8, CARDS_SHOWN = 3;
/** Far enough down that a card is wholly behind the foot. */
const CARD_HIDE = CARD_H + CARD_LIFT + 4;

/** A spring of stiffness 260 and damping 21, as a `linear()` curve: one
 *  overshoot of about 6% and a soft settle, the way a card with some weight
 *  lands. A WebKit too old for `linear()` gets the page's one overshooting
 *  bezier instead. `window.CSS`: this module's own CSS is a stylesheet. */
const SPRING = (() => {
  if (!window.CSS?.supports?.("transition-timing-function", "linear(0, 1)")) return { ease: "cubic-bezier(.3,1.6,.5,1)", ms: 520 };
  let x = 0, v = 0, t = 0;
  const pts = [0], dt = 1 / 1000;
  for (let next = 1 / 120; t < 2;) {
    v += (260 * (1 - x) - 21 * v) * dt; x += v * dt; t += dt;
    if (t >= next) { pts.push(+x.toFixed(4)); next += 1 / 120; }
    if (t > .12 && Math.abs(1 - x) < .0012 && Math.abs(v) < .02) break;
  }
  pts.push(1);
  return { ease: `linear(${pts.join(", ")})`, ms: Math.round(t * 1000) };
})();
const stillPlease = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

let cardsEl = null, cardsPane = null, cardsTimer = 0, cardsUp = 0;
/** The cards drawn, by panel, id and kind. */
const cardEls = new Map();

/** Where a forge keeps pull request `n` of the repository at `url`. */
function prUrl(url, n) {
  try {
    const h = new URL(url).hostname;
    return h === "gitlab.com" ? `${url}/-/merge_requests/${n}` : h === "bitbucket.org" ? `${url}/pull-requests/${n}`
      : h === "codeberg.org" ? `${url}/pulls/${n}` : `${url}/pull/${n}`;
  } catch { return null; }
}

/** Panel `v`'s cards on desk `d`, oldest first: its thread's PR, worked out
 *  here, then what its agent showed, less what has run its time. */
function cardsOf(d, v) {
  if (!v) return [];
  const out = [], now = Date.now() / 1000, t = paneThread(d, v.id), url = repoOf(d);
  const pr = t && t.pr && url && prUrl(url, t.pr);
  if (pr) {
    const ci = t.ci || "";
    out.push({ id: "pr", kind: "link", label: `PR #${t.pr} · ${t.merged ? "merged" : ci ? `checks ${ci}` : "open"}`, url: pr,
      host: repoName(url)?.host || "", at: 0, life: 0, by: "snyvi, from this panel's thread",
      tone: t.merged || /pass|success/i.test(ci) ? "ok" : /fail|error/i.test(ci) ? "bad" : "" });
  }
  for (const c of v.status.cards || []) if (!c.for || now < c.at + c.for) out.push({ ...c, life: c.for || 0 });
  return out;
}

/** What is inside a card: its mark, its words, where a link goes. */
function cardFill(el, c) {
  const { esc } = ctx;
  const mark = c.kind === "link" ? `<span class="dk-card-ic" aria-hidden="true">↗</span>` : `<span class="dk-card-tone${c.tone ? ` ${c.tone}` : ""}" aria-hidden="true"></span>`;
  el.innerHTML = `${mark}<span class="dk-card-tx">${esc(c.label)}</span>${c.host ? `<span class="dk-card-host">${esc(c.host)}</span>` : ""}`;
  el.dataset.tip = c.label;
  el.dataset.tipSub = [c.kind === "link" ? c.url : "", c.by || (c.life ? `for ${c.life}s` : "while this panel's session runs")].filter(Boolean).join(" · ");
  if (c.kind === "link") el.href = c.url;
  if (c.life) {
    const left = Math.max(0, c.at + c.life - Date.now() / 1000), bar = document.createElement("span");
    bar.className = "dk-card-drain";
    el.append(bar);
    bar.animate([{ transform: `scaleX(${left / c.life})` }, { transform: "scaleX(0)" }], { duration: left * 1000, easing: "linear", fill: "forwards" });
  }
}

function cardMake(c) {
  const el = document.createElement(c.kind === "link" ? "a" : "div");
  el.className = "dk-card";
  el.setAttribute("role", "listitem");
  if (c.kind === "link") { el.target = "_blank"; el.rel = "noopener noreferrer"; }
  cardFill(el, c);
  el.style.transform = `translateY(${CARD_HIDE}px)`;
  cardsEl.append(el);
  return el;
}

/** Card `el` to `y`, from wherever it is now -- mid-flight included -- on
 *  the spring, or on `ease` over `ms`. */
function cardMove(el, y, ease = SPRING.ease, ms = SPRING.ms) {
  const from = new DOMMatrixReadOnly(getComputedStyle(el).transform).m42;
  for (const a of el.getAnimations()) if (a.id === "move") a.cancel();
  el.style.transform = `translateY(${y}px)`;
  if (stillPlease() || Math.abs(from - y) < .5) return;
  el.animate([{ transform: `translateY(${from}px)` }, { transform: `translateY(${y}px)` }], { duration: ms, easing: ease }).id = "move";
}

function cardFade(el, to, ms) {
  const from = getComputedStyle(el).opacity;
  for (const a of el.getAnimations()) if (a.id === "fade") a.cancel();
  el.style.opacity = to;
  const a = el.animate([{ opacity: from }, { opacity: to }], { duration: stillPlease() ? 150 : ms, easing: "ease-out" });
  a.id = "fade";
  return a;
}

/** Up from behind the foot, to where it stands now. */
function cardIn(el) {
  if (!el.isConnected || el.$out) return;
  el.$up = true;
  cardMove(el, el.$y);
  cardFade(el, 1, 200);
}

/** Back down behind the foot, and gone. */
function cardOut(k, el, swap) {
  cardEls.delete(k);
  el.$out = true;
  const ms = swap ? 180 : 220;
  cardMove(el, CARD_HIDE, "cubic-bezier(.4,0,1,1)", ms);
  cardFade(el, 0, ms).finished.then(() => el.remove(), () => {});
}

/** The third card counts the ones behind it, and names them in its tip. */
function cardMore(el, rest) {
  let m = el.querySelector(".dk-card-more");
  if (!rest.length) { m?.remove(); return; }
  if (!m) { m = document.createElement("span"); m.className = "dk-card-more"; el.append(m); }
  m.textContent = `+${rest.length}`;
  m.dataset.tip = rest.map(c => c.label).join(" · ");
}

/** The stack, brought to what the focused panel holds: what left sinks,
 *  what came rises (a burst one after another, bottom first), what moved
 *  springs to its new place, and a card updated in place is only rewritten.
 *  Called with the foot's line (`meta`), so on every frame of the focused
 *  panel and every redraw of the rail; a call that changes nothing touches
 *  nothing. */
function cards() {
  const d = current(), v = d ? views.get(focused) : null;
  if (!cardsEl || !cardsEl.isConnected) {
    cardsEl = Object.assign(document.createElement("div"), { id: "dk-cards" });
    cardsEl.setAttribute("role", "list");
    cardsEl.setAttribute("aria-label", "Cards");
    ctx.metaEl.before(cardsEl);
  }
  const list = d ? cardsOf(d, v) : [], shown = list.slice(-CARDS_SHOWN).reverse();
  const swap = cardsPane !== (v ? v.id : null);
  cardsPane = v ? v.id : null;
  const key = c => `${v.id}:${c.id}:${c.kind}`, want = new Set(shown.map(key));
  for (const [k, el] of cardEls) if (!want.has(k)) cardOut(k, el, swap);
  let rising = 0;
  shown.forEach((c, i) => {
    const k = key(c), y = -i * (CARD_H + CARD_GAP), sig = JSON.stringify([c.label, c.url, c.host, c.tone, c.at, c.life]);
    let el = cardEls.get(k);
    if (!el) {
      el = cardMake(c);
      cardEls.set(k, el);
      el.$y = y; el.$sig = sig;
      setTimeout(() => cardIn(el), (swap ? 120 : 0) + rising++ * 60);
    } else {
      if (el.$sig !== sig) { cardFill(el, c); el.$sig = sig; }
      if (el.$y !== y) { el.$y = y; if (el.$up) cardMove(el, y); }
    }
    cardMore(el, i === shown.length - 1 ? list.slice(0, -CARDS_SHOWN) : []);
  });
  // The corner stands on the cards: it moves when their count does.
  if (shown.length !== cardsUp) { cardsUp = shown.length; if (oyEl?.isConnected) oyPlace(oyEl); }
  // The next card whose time runs out takes itself off then.
  clearTimeout(cardsTimer);
  const next = Math.min(...list.filter(c => c.life).map(c => c.at + c.life));
  if (Number.isFinite(next)) cardsTimer = setTimeout(cards, Math.max(0, next * 1000 - Date.now()) + 50);
}

/** How far above the foot the cards reach: where the On you corner stands. */
const cardsLift = () => cardsUp ? cardsUp * (CARD_H + CARD_GAP) - CARD_GAP + CARD_LIFT : 0;

/** Off the desk: no stack, no timer. */
function cardsDrop() {
  clearTimeout(cardsTimer);
  cardEls.clear();
  cardsEl?.remove();
  cardsEl = null; cardsPane = null; cardsUp = 0;
}

const CARDS_CSS = `
#dk-cards { position: absolute; left: 8px; right: 10px; bottom: var(--foot-h); height: 0; z-index: 1; pointer-events: none; }
:root:not([data-view="desk"]) #dk-cards { display: none; }
/* The list always ends in room for three cards, so its last line can be
 * scrolled clear of them; that room never changes, so nothing moves. */
:root[data-view="desk"] #toc { padding-bottom: calc(14px + 3 * 36px + 8px); }
.dk-card { position: absolute; left: 0; right: 0; bottom: 8px; height: 30px; display: flex; align-items: center; gap: 7px; padding: 0 10px;
  border-radius: var(--r-md); background: var(--bg-raise); border: 1px solid var(--rule-2); box-shadow: var(--shadow-1);
  color: var(--fg-2); font-size: var(--fs-small); white-space: nowrap; pointer-events: auto; text-decoration: none; opacity: 0; }
a.dk-card:hover, a.dk-card:focus-visible { border-color: var(--accent); color: var(--fg); text-decoration: none; }
.dk-card-ic { flex: none; color: var(--accent); }
.dk-card-tx { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
.dk-card-host { flex: none; margin-left: auto; font-family: var(--mono); font-size: var(--fs-micro); color: var(--fg-3); }
.dk-card-tone { flex: none; width: 7px; height: 7px; border-radius: 50%; background: var(--fg-3); }
.dk-card-tone.ok { background: var(--ok); }
.dk-card-tone.warn { background: var(--warn); }
.dk-card-tone.bad { background: var(--danger); }
/* Under way is a ring, not a colour: the accent can be any hue, and a
 * Crimson one filled in read as "bad". */
.dk-card-tone.run { background: none; box-shadow: inset 0 0 0 1.5px var(--fg-2); }
/* The time a card has left, draining right to left under it. */
.dk-card-drain { position: absolute; left: 10px; right: 10px; bottom: 3px; height: 1.5px; border-radius: var(--r-xs); background: var(--accent); opacity: .55; transform-origin: left center; pointer-events: none; }
.dk-card-more { position: absolute; top: -7px; right: 8px; padding: 0 6px; border-radius: var(--r-pill); background: var(--rule-2); color: var(--fg-2); font-size: var(--fs-micro); font-weight: 600; line-height: 14px; }
`;
