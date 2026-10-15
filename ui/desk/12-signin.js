/* ui/desk/12-signin.js: a part of desk.js, one module. build.rs joins ui/desk/*.js in name
 * order (src/strip.rs `source`); SNYVI_UI_DIR serves the same join. */
// ---------- signing a Claude account in ----------
/*
 * One click (plan 7ead8a629d): snyvi runs `claude setup-token` out of sight
 * and keeps the token it prints (`accounts::signin`). The page asks how it
 * is going each second while one is open, and is told the sign-in page's
 * address and how it ended, never the token. The keys sheet's "+ Claude
 * account" and a panel's strip -- Add another account, Renew -- both start
 * one with `signIn`, and each draws its progress where it asked
 * (`signRow`). One at a time, as the daemon has it: a second ends the first.
 *
 * The Code field is the rare case, the browser unable to hand the approval
 * back, so it waits behind a small "Page shows a code?" that comes after a
 * while. Paste is the rarer one, a sign-in that cannot work at all, so only
 * a failure offers it, and only where there is a form for it.
 */
const SIGN_CODE_AFTER_MS = 15000;
/** The sign-in as the daemon last said it; the promise `signIn` handed out;
 *  when it started; whether the reader asked for the Code field. */
let signing = null, signTimer = 0, signWait = null, signSince = 0, signCoded = false;
/** Which press the answer on its way is for: a Cancel before the daemon
 *  had even started the sign-in moves it on, and that answer then ends the
 *  sign-in it started rather than drawing it. */
let signGen = 0;
/** Where progress is drawn: the sheet's row, a panel's strip. */
const signRows = new Set();
const signOpen = s => !!s && /^(starting|waiting|saving)$/.test(s.phase);

/** Sign in a new account, or `renew` one's token: the account's id once its
 *  token is kept. Rejects with the reason when it fails, and with
 *  `cancelled` set when the reader, or a second sign-in, ended it. */
function signIn({ renew } = {}) {
  signWait?.no(Object.assign(new Error("cancelled"), { cancelled: true }));
  const p = new Promise((ok, no) => { signWait = { ok, no }; });
  signSince = Date.now(); signCoded = false;
  signing = { phase: "starting" }; signDraw();
  const gen = ++signGen;
  ctx.api("/api/accounts/signin", renew ? { renew } : {})
    .then(j => {
      if (gen !== signGen) return ctx.api("/api/accounts/signin/cancel", {}).catch(() => {});
      signing = j.signin; signPoll();
    }, e => { if (gen === signGen) { signing = { phase: "failed", error: ctx.sayErr(e).why }; signPoll(); } });
  return p;
}

/** Ask again in a second while it is open; settle the promise once it ends. */
function signPoll() {
  clearTimeout(signTimer);
  signDraw();
  if (!signOpen(signing)) return signSettle();
  signTimer = setTimeout(async () => {
    try { signing = (await ctx.api("/api/accounts/signin")).signin; } catch {}
    if (signing?.phase === "done") await ctx.refresh();
    signPoll();
  }, 1000);
}
function signSettle() {
  const w = signWait, s = signing;
  if (!w) return;
  signWait = null;
  if (s?.phase === "done") w.ok(s.account);
  else w.no(Object.assign(new Error(s?.error || "cancelled"), { cancelled: !s || s.phase === "cancelled" }));
}
/** The sign-in a sheet found open when it opened: drawn, and followed. */
async function signFollow() {
  try { signing = (await ctx.api("/api/accounts/signin")).signin; } catch { return; }
  signPoll();
}

async function signCancel() {
  signGen++;
  try { await ctx.api("/api/accounts/signin/cancel", {}); } catch {}
  if (signOpen(signing) && signing.phase !== "saving") signing = { ...signing, phase: "cancelled" };
  signPoll();
}

/** Draw the sign-in's progress in `el` from now on. `fail` is what a failure
 *  offers there: `{ label, run }`, the sheet's paste, a strip's try again. */
function signRow(el, fail) {
  el.$signFail = fail;
  signRows.add(el);
  if (!el.$signWired) {
    el.$signWired = true;
    el.addEventListener("click", e => {
      const b = e.target.closest("[data-sg]");
      if (!b) return;
      const k = b.dataset.sg;
      if (k === "open") { if (/^https:\/\//.test(signing?.url || "")) openLink(signing.url); }
      else if (k === "x") signCancel();
      else if (k === "code") { signCoded = true; signDraw(); el.querySelector("input[name=code]")?.focus(); }
      else if (k === "send") signSendCode(el);
      else if (k === "fail") el.$signFail?.run();
    });
    // What is typed in the Code field is the field's: not the panel's, not
    // the sheet's keys.
    el.addEventListener("keydown", e => {
      if (!e.target.matches("input[name=code]")) return;
      if (e.key !== "Escape") e.stopPropagation();
      if (e.key === "Enter") { e.preventDefault(); signSendCode(el); }
    });
  }
  signDraw();
}

function signDraw() {
  for (const el of signRows) {
    if (!el.isConnected) { signRows.delete(el); continue; }
    const html = signHtml(signing, el.$signFail);
    if (el.$signHtml === html) continue;
    el.innerHTML = html; el.$signHtml = html; el.hidden = !html;
  }
}

function signHtml(s, fail) {
  const { esc } = ctx;
  if (!s || s.phase === "done" || s.phase === "cancelled") return "";
  if (s.phase === "failed") return `<span class="sg-say">Couldn't sign in · ${esc(s.error || "it ended")}</span>${fail ? `<button type="button" class="sg-link" data-sg="fail">${esc(fail.label)}</button>` : ""}`;
  if (s.phase === "saving") return `<span class="sg-say">Keeping it…</span>`;
  const x = `<button type="button" data-sg="x">Cancel</button>`;
  if (s.phase === "starting") return `<span class="sg-say">Opening Claude's sign-in…</span>${x}`;
  const late = Date.now() - signSince > SIGN_CODE_AFTER_MS;
  const code = signCoded
    ? `<input name="code" spellcheck="false" autocomplete="off" placeholder="the code the page shows" aria-label="The code the sign-in page shows"><button type="button" data-sg="send">Send</button>`
    : late ? `<button type="button" class="sg-link" data-sg="code">Page shows a code?</button>` : "";
  return `<span class="sg-say">Approve in your browser${s.renew ? ` as ${esc(s.label)}` : ""}</span><button type="button" class="sg-link" data-sg="open" data-tip="If the browser did not open">Open the page</button>${code}${x}`;
}

async function signSendCode(el) {
  const inp = el.querySelector("input[name=code]");
  if (!inp || !inp.value.trim()) return;
  try { await ctx.api("/api/accounts/signin/code", { code: inp.value.trim() }); inp.value = ""; inp.placeholder = "sent · waiting for Claude"; }
  catch (e) { ctx.toast("Could not send the code", e); }
}

const SIGNIN_CSS = `
/* A sign-in's progress, one line wherever it is drawn: the keys sheet's
   row, a panel's strip. */
.sg-row { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 10px; color: var(--fg-2); font-size: var(--fs-small); }
.sg-row[hidden] { display: none; }
.sg-row button { padding: 0; border: 0; background: none; font: inherit; color: var(--fg-3); cursor: pointer; }
.sg-row button:hover { color: var(--fg); }
.sg-row .sg-link { color: var(--accent); }
.sg-row input { flex: 1; min-width: 12ch; padding: 1px 6px; border: 1px solid var(--rule-2); border-radius: var(--r-xs); background: var(--bg); font: inherit; color: var(--fg); }
.pn-switch .sg-row { flex: 1; min-width: 0; flex-wrap: nowrap; font-size: inherit; }
.pn-switch .sg-row .sg-say { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
`;
