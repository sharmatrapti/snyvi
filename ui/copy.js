/* Copy all (#165): the page's whole text, to take somewhere else.
 *
 * A chunk since 1.44, so first paint does not carry it (bench/bytes.mjs).
 * The page fetches it once it is idle, as it fetches nav.js: by the time a
 * hand reaches the button beside Width, or presses y, it is here, and the
 * click reaches the clipboard with nothing awaited before it, which WebKit
 * asks. A click before it came waits for it, and WebKit may refuse that one.
 *
 * Two kinds at once, and what it is pasted into takes the one it reads: the
 * source as it was sent (its raw route, the same bytes Open source shows)
 * for an agent, a terminal, GitHub; and, for Markdown, the page as drawn,
 * for a mail or a document. The source is fetched inside the click's
 * ClipboardItem, as a promise: WebKit forgets the click once a fetch has
 * been awaited, and writeText after it is refused there.
 */

const NO_TEXT = /^(image|video|audio|binary)$/;
let c = null;

export function init(ctx) { c = ctx; return { copyAll }; }

export async function copyAll(btn) {
  const { state, rawUrl, toast, keyHint } = c, $ = s => document.querySelector(s);
  const file = state.view === "browse" && state.browseRoot && state.browsePath;
  const url = file ? rawUrl(state.browseRoot.id, state.browsePath) : state.view === "doc" && state.doc ? `/api/docs/${state.doc.id}/raw` : "";
  const kind = file ? ($("#doc article")?.className.match(/kind-(\w+)/) || [])[1] : state.doc?.kind;
  if (!url || NO_TEXT.test(kind || "") || btn?.dataset.copied) return;
  const said = file ? state.browsePath.split("/").pop() : state.doc.title;
  const text = fetch(url).then(r => r.ok ? r.text() : Promise.reject(new Error(r.status)));
  const art = $("#doc article.kind-markdown");
  let html = "";
  if (art) {
    // The page as drawn, less what is only the page's: its buttons.
    const cl = art.cloneNode(true);
    cl.querySelectorAll("button, .copy").forEach(x => x.remove());
    html = cl.innerHTML;
  }
  try {
    if (!window.ClipboardItem) throw 0;
    const item = { "text/plain": text.then(t => new Blob([t], { type: "text/plain" })) };
    if (html) item["text/html"] = new Blob([html], { type: "text/html" });
    await navigator.clipboard.write([new ClipboardItem(item)]);
  } catch {
    // No ClipboardItem, or the formatted kind refused: the source alone.
    try { await navigator.clipboard.writeText(await text); }
    catch { return toast("Could not copy", { sub: `select and press ${keyHint("mod+c")}`, at: btn }); }
  }
  if (!btn?.isConnected || btn.offsetParent === null) return toast("Copied", { sub: said });
  btn.dataset.copied = 1; btn.classList.add("did");
  setTimeout(() => { delete btn.dataset.copied; btn.classList.remove("did"); }, 1200);
}
