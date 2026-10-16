/* ui/desk/13-pipeline.js: a part of desk.js, one module. build.rs joins ui/desk/*.js in name
 * order (src/strip.rs `source`); SNYVI_UI_DIR serves the same join. */
// ---------- pipelines ----------

/* A pipeline is a panel working through a plan the reader can watch
 * (src/pipeline.rs, plan 2a0edf418f): a list of their steps, one step looped,
 * or a goal a planner works towards, each stage a fresh Claude in that
 * panel. Here: the chip in the panel's head that says where it is, and the
 * sheet it opens -- the steps, who wrote each, Go and Skip on the planner's,
 * Start, Pause, Resume, Stop -- and the editor.
 *
 * The chip lives in the head, a row of fixed height: it never moves the
 * terminal, and its width is held while the words in it change, so a stage
 * starting moves nothing beside it either. The sheet lays over the panel's
 * top, like the panel's other strips. */

/** Hidden since 1.43 (2026-10-16): the reader set the feature aside while a
 *  simpler shape is worked out (the Away brainstorm, docs 5e66a26179 and
 *  9074c8ca08). The daemon, its routes and the runner stay as they are; the
 *  window just stops offering it -- no New pipeline…, no New… or Edit… in
 *  the sheet, and nothing of a pipeline that has ended. One still going,
 *  made before the hiding, keeps its chip and its sheet: a pipeline that
 *  runs where it cannot be seen or stopped would be worse than one that
 *  shows. Flip this to bring it back whole. */
const PIPES_SHOWN = false;

/** This desk's pipelines, as the daemon last said them, and which desk. */
let pipes = [], pipesAt = null;
/** The sheet open over one panel: { pane, edit: pipeline id | 0 for new | null }. */
let pipeOpen = null;
/** What the sheet last failed at, said in it until the next action. */
let pipeErr = "";

/** This desk's pipelines, read again: on open and on every change the
 *  daemon announces (the `desknotes` event, `pipeline_run::changed`). */
async function pipesGet(id) {
  let j;
  try { j = await ctx.api(`/api/desks/${id}/pipelines`); } catch { return; }
  if (id !== deskId) return;
  pipes = (j.pipelines || []).filter(p => !p.hidden_at);
  pipesAt = id;
  for (const v of views.values()) pipeChip(v);
  if (pipeOpen) pipeSheet();
}

/** The pipeline a panel shows: the one it is working through, else its
 *  newest -- while pipelines are hidden, only one still going. */
const pipeOf = pane => pipes.find(p => p.pane === pane && ["running", "waiting", "paused"].includes(p.state)) || (PIPES_SHOWN ? pipes.find(p => p.pane === pane) : undefined);

const PIPE_SAYS = { draft: "ready", running: "running", waiting: "Go?", paused: "paused", done: "done", stopped: "stopped" };

/** The head's chip: name, stage count, state. Absent on a panel with none. */
function pipeChip(v) {
  pipeStyle();
  const hd = v.el.querySelector(".pn-head"), p = pipesAt === deskId ? pipeOf(v.id) : null;
  let c = hd.querySelector(".pn-pipe");
  if (!p) { if (c) c.remove(); return; }
  if (!c) {
    c = document.createElement("button");
    c.type = "button"; c.className = "pn-pipe";
    hd.querySelector(".pn-state").before(c);
    c.addEventListener("click", e => { e.stopPropagation(); pipeToggle(v.id); });
  }
  const ran = p.steps.filter(s => ["done", "failed", "running", "stopped"].includes(s.state)).length;
  const run = p.steps.find(s => s.state === "running");
  c.className = `pn-pipe ${p.state}`;
  c.textContent = `◇ ${p.name} · ${ran}/${p.kind === "list" ? p.steps.filter(s => s.state !== "skipped").length : p.caps.stages} · ${PIPE_SAYS[p.state] || p.state}`;
  c.dataset.tip = p.why || (run ? `Stage ${run.n}: ${run.text.slice(0, 120)}` : "Pipeline");
  c.setAttribute("aria-label", `Pipeline ${p.name}, ${p.state}`);
}

/** The panel menu's "Pipeline…": the sheet, or the editor for a new one. */
function pipeItems(v) {
  if (!PIPES_SHOWN && !pipeOf(v.id)) return [];
  return [{ label: pipeOf(v.id) ? "Pipeline…" : "New pipeline…", run: () => pipeToggle(v.id, !pipeOf(v.id)) }];
}

function pipeToggle(pane, fresh) {
  if (pipeOpen && pipeOpen.pane === pane && !fresh) { pipeClose(); return; }
  pipeOpen = { pane, edit: fresh ? 0 : null };
  pipeErr = "";
  pipeSheet();
}

function pipeClose() {
  document.querySelector(".pn-pipes")?.remove();
  pipeOpen = null;
}

/** The sheet over the panel: the pipeline, or the editor. */
function pipeSheet() {
  const v = pipeOpen && views.get(pipeOpen.pane);
  if (!v) { pipeClose(); return; }
  pipeStyle();
  let el = v.el.querySelector(".pn-pipes");
  if (!el) {
    document.querySelector(".pn-pipes")?.remove();
    el = document.createElement("section");
    el.className = "pn-pipes";
    el.setAttribute("role", "dialog");
    el.setAttribute("aria-label", "Pipeline");
    v.el.append(el);
    el.addEventListener("click", pipeClick);
    el.addEventListener("keydown", e => { if (e.key === "Escape") { e.stopPropagation(); pipeClose(); v.body.focus(); } });
  }
  // Just removed from view: its Undo, where the ✕ was, for BACK_MS.
  if (pipeOpen.gone) {
    el.innerHTML = `<p class="pp-gone" role="status">${ctx.esc(pipeOpen.gone.name)} is removed from view. <button type="button" data-pp="unhide">Undo</button></p>`;
    return;
  }
  const p = pipeOpen.edit == null ? pipeOf(v.id) : pipes.find(x => x.id === pipeOpen.edit);
  // Hidden, the sheet is only ever a pipeline still going: one that has just
  // ended takes its sheet with it, rather than leaving the editor for a new one.
  if (!PIPES_SHOWN && (!p || pipeOpen.edit != null)) { pipeClose(); return; }
  el.innerHTML = (pipeOpen.edit != null || !p ? pipeEditor(p) : pipeView(p)) +
    (pipeErr ? `<p class="pp-err" role="alert">${ctx.esc(pipeErr)}</p>` : "");
}

const AUTHOR = { reader: "you", planner: "planner" };

/** A pipeline, read: where it is, why, its steps, and what can be done. */
function pipeView(p) {
  const e = ctx.esc, s = p.state;
  const btn = (a, label, extra = "") => `<button type="button" data-pp="${a}"${extra}>${label}</button>`;
  const acts = [
    s === "draft" && btn("start", "▶ Start", " class=\"pri\""),
    ["paused", "stopped", "done"].includes(s) && btn("resume", s === "paused" ? "▶ Resume" : "▶ Go on"),
    ["running", "waiting"].includes(s) && btn("pause", "Pause"),
    (["running", "waiting", "paused", "draft"].includes(s)) && btn("stop", "Stop"),
    PIPES_SHOWN && s !== "running" && btn("edit", "Edit…"),
    p.report && btn("report", "Report"),
    !["running", "waiting"].includes(s) && btn("hide", "✕", ` data-tip="Remove from view" aria-label="Remove this pipeline from view"`),
    PIPES_SHOWN && btn("new", "New…"),
  ].filter(Boolean).join("");
  const steps = p.steps.map(x => {
    const go = x.state === "proposed" ? `<span class="pp-go">${btn("go", "Go", ` data-step="${x.id}" class="pri"`)}${btn("skip", "Skip", ` data-step="${x.id}"`)}</span>` : "";
    const meta = [x.ctx_peak ? `${x.ctx_peak}%` : "", x.commits ? `${x.commits.split("\n").length} commit${x.commits.includes("\n") ? "s" : ""}` : "", x.handoff > 0 ? `handed on ${x.handoff}×` : ""].filter(Boolean).join(" · ");
    return `<li class="pp-step ${x.state}"><span class="pp-n">${x.n}</span><span class="pp-by ${x.author}">${AUTHOR[x.author] || x.author}${x.kind === "goal" ? " · goal" : ""}</span>` +
      `<span class="pp-text" data-tip="${e(x.text.slice(0, 400))}">${e(x.text.split("\n")[0].slice(0, 200))}</span><span class="pp-st">${e(x.state)}</span>` +
      (x.why ? `<span class="pp-why">${e(x.why)}</span>` : "") + (meta ? `<span class="pp-meta">${e(meta)}</span>` : "") + go + `</li>`;
  }).join("");
  const kind = { list: "your list", loop: "a loop", goal: "a goal" }[p.kind] || p.kind;
  return `<header class="pp-head"><b>◇ ${e(p.name)}</b><span class="pp-kind">${kind}${p.kind === "goal" ? (p.autopilot ? " · autopilot" : " · Go on each step") : ""}</span><span class="pp-state ${s}">${e(PIPE_SAYS[s] || s)}</span>` +
    `<button type="button" class="pp-x" data-pp="close" aria-label="Close">✕</button></header>` +
    (p.why ? `<p class="pp-whyall">${e(p.why)}${p.resume_at ? ` · goes on at ${new Date(p.resume_at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}` : ""}</p>` : "") +
    (p.goal ? `<p class="pp-goal"><span>${p.kind === "loop" ? "Each cycle" : "Goal"}</span> ${e(p.goal)}</p>` : "") +
    (p.branch ? `<p class="pp-branch">${e(p.branch)} · ${e(p.folder)}</p>` : "") +
    `<ol class="pp-steps">${steps || `<li class="pp-none">${p.kind === "goal" ? "The planner writes the first step at Start." : "No steps yet."}</li>`}</ol>` +
    `<div class="pp-acts">${acts}</div>`;
}

/** The editor: a new pipeline on this panel, or one that is not running. */
function pipeEditor(p) {
  const e = ctx.esc, c = (p && p.caps) || { stages: 12, cycles: 6, minutes: 240, five_hour: 80, diff_lines: 1500 };
  const kind = (p && p.kind) || "list";
  const queued = p ? p.steps.filter(s => s.state === "queued") : [];
  const radio = (k, label) => `<label><input type="radio" name="pp-kind" value="${k}"${kind === k ? " checked" : ""}> ${label}</label>`;
  const num = (n, label, val, min, max) => `<label class="pp-cap">${label}<input type="number" name="${n}" value="${val}" min="${min}" max="${max}"></label>`;
  return `<header class="pp-head"><b>${p ? "Edit pipeline" : "New pipeline"}</b><span class="pp-kind">on panel ${views.get(pipeOpen.pane)?.pane.slot || ""}</span>` +
    `<button type="button" class="pp-x" data-pp="close" aria-label="Close">✕</button></header>` +
    `<form class="pp-form" data-id="${p ? p.id : 0}">` +
    `<label class="pp-wide">Name<input name="name" maxlength="60" required value="${e((p && p.name) || "")}" placeholder="Trim first paint" spellcheck="false"></label>` +
    `<div class="pp-kinds" role="radiogroup" aria-label="Where the next step comes from">${radio("list", "My steps, in order")}${radio("loop", "One step, again")}${radio("goal", "A goal: a planner writes each step")}</div>` +
    `<label class="pp-wide pp-for-list">Steps, one to a block; a line of <code>---</code> between them. A step starting <code>/goal</code> keeps going until Claude Code's judge finds it met.` +
    `<textarea name="steps" rows="6" spellcheck="false" placeholder="Run node bench/bytes.mjs and say the three biggest offenders&#10;---&#10;/goal first paint under 58 KB with bench/bytes.mjs --check green. Stop after 10 turns.">${e(queued.map(s => (s.kind === "goal" ? "/goal " : "") + s.text).join("\n---\n"))}</textarea></label>` +
    `<label class="pp-wide pp-for-goal">The goal, or for a loop the step each cycle runs<textarea name="goal" rows="3" spellcheck="false" placeholder="Every function under 120 lines (node bench/size.mjs --check), cargo test green after each stage">${e((p && p.goal) || "")}</textarea></label>` +
    `<label class="pp-wide">Rules for every stage <span class="pp-fine">(and the planner)</span><textarea name="rules" rows="2" spellcheck="false" placeholder="Run cargo test before committing. Never touch ui/mermaid.js.">${e((p && p.rules) || "")}</textarea></label>` +
    `<div class="pp-row"><label class="pp-auto pp-for-plan"><input type="checkbox" name="autopilot"${p && p.autopilot ? " checked" : ""}> Autopilot: the planner's steps run without my Go</label>` +
    `<label class="pp-cap pp-for-plan">Planner<select name="model">${["sonnet", "haiku", "opus"].map(m => `<option${((p && p.planner_model) || "sonnet") === m ? " selected" : ""}>${m}</option>`).join("")}</select></label></div>` +
    `<div class="pp-row">${num("ctx", "Hand on at context %", (p && p.ctx_limit) || 70, 20, 95)}${num("stages", "Stages", c.stages, 1, 100)}${num("cycles", "Cycles", c.cycles, 1, 100)}${num("minutes", "Minutes", c.minutes, 5, 1440)}${num("five_hour", "Stop at 5-hour %", c.five_hour, 10, 100)}${num("diff_lines", "Lines a stage may change", c.diff_lines, 10, 100000)}</div>` +
    `<p class="pp-fine">Stages run in a worktree beside this folder, on a <code>pipeline/</code> branch, with Claude Code in auto mode. They commit there and never push. Each stage is a fresh session, on this desk's Claude account.</p>` +
    `<div class="pp-acts"><button type="submit" class="pri">Save</button>${p ? `<button type="button" data-pp="view">Cancel</button>` : ""}</div></form>`;
}

/** The editor's form, as the daemon takes it. */
function pipeRead(f) {
  const kind = f.querySelector("[name=pp-kind]:checked")?.value || "list";
  const p = pipes.find(x => x.id === Number(f.dataset.id));
  const queued = p ? p.steps.filter(s => s.state === "queued") : [];
  const steps = kind === "list" ? f.steps.value.split(/\n-{3,}\n/).map(t => t.trim()).filter(Boolean).map((t, i) => {
    const goal = /^\/goal\s/.test(t);
    return { id: queued[i] ? queued[i].id : undefined, kind: goal ? "goal" : "prompt", text: goal ? t.replace(/^\/goal\s+/, "") : t };
  }) : [];
  const n = k => Number(f[k].value);
  return {
    name: f.name.value.trim(), kind, steps, goal: kind === "list" ? "" : f.goal.value.trim(), rules: f.rules.value.trim(),
    autopilot: kind === "goal" && f.autopilot.checked, planner_model: f.model.value, ctx_limit: n("ctx"),
    caps: { stages: n("stages"), cycles: n("cycles"), minutes: n("minutes"), five_hour: n("five_hour"), diff_lines: n("diff_lines") },
  };
}

/** A press in the sheet. */
async function pipeClick(e) {
  const b = e.target.closest("[data-pp]");
  if (!b) return;
  const a = b.dataset.pp, p = pipeOf(pipeOpen.pane);
  if (a === "close") { pipeClose(); return; }
  if (a === "new") { pipeOpen.edit = 0; pipeErr = ""; pipeSheet(); return; }
  if (a === "edit") { pipeOpen.edit = p.id; pipeErr = ""; pipeSheet(); return; }
  if (a === "view") { pipeOpen.edit = null; pipeErr = ""; pipeSheet(); return; }
  if (a === "report") { ctx.read(p.report); return; }
  if (a === "unhide") {
    const g = pipeOpen.gone;
    pipeOpen.gone = null;
    try { await ctx.api(`/api/desks/${deskId}/pipelines/${g.id}/unhide`, {}); } catch (err) { pipeErr = `Could not bring it back · ${ctx.sayErr(err).why}`; }
    await pipesGet(deskId);
    return;
  }
  if (!p) return;
  b.disabled = true;
  try {
    await ctx.api(`/api/desks/${deskId}/pipelines/${p.id}/${a}`, b.dataset.step ? { step: Number(b.dataset.step) } : {});
    pipeErr = "";
    if (a === "hide") {
      const gone = pipeOpen.gone = { id: p.id, name: p.name };
      setTimeout(() => { if (pipeOpen && pipeOpen.gone === gone) pipeClose(); }, BACK_MS);
    }
  } catch (err) { pipeErr = `Could not ${a} it · ${ctx.sayErr(err).why}`; }
  await pipesGet(deskId);
}

/** The editor's Save. */
async function pipeSave(e) {
  const f = e.target.closest(".pp-form");
  if (!f) return;
  e.preventDefault();
  const body = pipeRead(f), id = Number(f.dataset.id);
  const btn = f.querySelector("[type=submit]");
  btn.disabled = true;
  try {
    if (id) await ctx.api(`/api/desks/${deskId}/pipelines/${id}/edit`, body);
    else await ctx.api(`/api/desks/${deskId}/pipelines`, { pane: pipeOpen.pane, ...body });
    pipeOpen.edit = null; pipeErr = "";
  } catch (err) { pipeErr = ctx.sayErr(err).why; btn.disabled = false; }
  await pipesGet(deskId);
  if (pipeErr) pipeSheet();
}

document.addEventListener("submit", pipeSave, true);

function pipeStyle() {
  if (document.getElementById("pipe-css")) return;
  const s = document.createElement("style");
  s.id = "pipe-css";
  s.textContent = PIPE_CSS;
  document.head.append(s);
}

/* The chip takes the head's own line height and no border, so the head is
 * exactly as tall with it as without: a panel's terminal is never resized
 * by a pipeline appearing in its head. */
const PIPE_CSS = `
.pn-pipe { flex: none; min-width: 16ch; max-width: 34ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: inherit; line-height: inherit;
  color: var(--fg-2); background: var(--accent-bg); border: 0; border-radius: var(--r-pill); padding: 0 8px; cursor: pointer; }
.pn-pipe.running { color: var(--accent-d); }
.pn-pipe.waiting { color: var(--bg); background: var(--accent); }
.pn-pipe.paused { color: var(--warn); }
.pn-pipe.done, .pn-pipe.stopped, .pn-pipe.draft { color: var(--fg-3); background: none; box-shadow: inset 0 0 0 1px var(--rule); }
.pn-pipes { position: absolute; z-index: 5; top: 28px; right: 8px; width: min(620px, calc(100% - 16px)); max-height: calc(100% - 36px); overflow: auto;
  background: var(--bg-raise); color: var(--fg); border: 1px solid var(--rule); border-radius: var(--r-md); box-shadow: var(--shadow-3); padding: 12px 14px; font-size: 13px; }
.pp-head { display: flex; align-items: baseline; gap: 10px; margin-bottom: 8px; }
.pp-head b { font-size: 13px; }
.pp-kind, .pp-fine, .pp-branch, .pp-meta { color: var(--fg-3); font-size: 11px; }
.pp-state { margin-left: auto; font-size: 11px; color: var(--fg-2); }
.pp-state.running { color: var(--accent-d); }
.pp-state.paused { color: var(--warn); }
.pp-x { background: none; border: 0; color: var(--fg-3); cursor: pointer; font: inherit; }
.pp-whyall { margin: 0 0 8px; color: var(--fg-2); }
.pp-goal { margin: 0 0 6px; }
.pp-goal span { color: var(--fg-3); margin-right: 4px; }
.pp-branch { margin: 0 0 8px; font-family: var(--pn-font); }
.pp-steps { list-style: none; margin: 0 0 10px; padding: 0; display: flex; flex-direction: column; gap: 4px; }
.pp-step { display: grid; grid-template-columns: 2ch 9ch 1fr auto; gap: 2px 8px; align-items: baseline; padding: 4px 6px; border-radius: var(--r-sm); }
.pp-step.running { background: var(--accent-bg); }
.pp-step.proposed { box-shadow: inset 0 0 0 1px var(--accent); }
.pp-step.skipped .pp-text { text-decoration: line-through; color: var(--fg-3); }
.pp-n { color: var(--fg-3); text-align: right; }
.pp-by { color: var(--fg-3); font-size: 11px; }
.pp-by.planner { color: var(--accent-d); }
.pp-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pp-st { color: var(--fg-3); font-size: 11px; }
.pp-why, .pp-meta, .pp-go { grid-column: 3 / 5; }
.pp-why { color: var(--fg-2); font-style: italic; }
.pp-go { display: flex; gap: 6px; }
.pp-none { color: var(--fg-3); }
.pp-acts { display: flex; flex-wrap: wrap; gap: 6px; }
.pn-pipes button:not(.pp-x) { font: inherit; padding: 3px 10px; border-radius: var(--r-sm); border: 1px solid var(--rule); background: var(--bg); color: var(--fg); cursor: pointer; }
.pn-pipes button.pri { background: var(--accent); color: var(--bg); border-color: var(--accent); }
.pn-pipes button:disabled { opacity: .5; cursor: default; }
.pp-err { color: var(--danger); margin: 8px 0 0; }
.pp-form { display: flex; flex-direction: column; gap: 8px; }
.pp-form label { display: flex; flex-direction: column; gap: 3px; }
.pp-form input, .pp-form textarea, .pp-form select { font: inherit; color: var(--fg); background: var(--bg); border: 1px solid var(--rule); border-radius: var(--r-sm); padding: 4px 6px; }
.pp-form textarea { font-family: var(--pn-font); font-size: 12px; resize: vertical; }
.pp-kinds { display: flex; flex-wrap: wrap; gap: 4px 14px; }
.pp-kinds label, .pp-form .pp-auto { flex-direction: row; align-items: center; gap: 6px; }
.pp-row { display: flex; flex-wrap: wrap; gap: 8px 12px; align-items: end; }
.pp-cap { font-size: 11px; color: var(--fg-3); }
.pp-cap input { width: 9ch; }
.pp-form:has([value=list]:checked) .pp-for-goal, .pp-form:not(:has([value=list]:checked)) .pp-for-list,
.pp-form:not(:has([value=goal]:checked)) .pp-for-plan { display: none; }
`;
