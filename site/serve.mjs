/* A preview of site/out the way Cloudflare serves it: /install is
   install.html, a miss is 404.html, and _headers' first block applies.
     node site/serve.mjs [port]     (default 7801; never 7777) */
import { createServer } from "node:http";
import { readFileSync, existsSync, statSync } from "node:fs";
import { join, extname, dirname } from "node:path";
import { fileURLToPath } from "node:url";
const OUT = join(dirname(fileURLToPath(import.meta.url)), "out");
const PORT = +(process.argv[2] || 7801);
const TYPES = { ".html": "text/html; charset=utf-8", ".css": "text/css", ".js": "text/javascript", ".json": "application/json", ".svg": "image/svg+xml", ".webp": "image/webp", ".png": "image/png", ".woff2": "font/woff2", ".mp4": "video/mp4", ".xml": "application/xml", ".txt": "text/plain" };
const csp = (readFileSync(join(OUT, "_headers"), "utf8").match(/Content-Security-Policy: (.*)/) || [])[1];
createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, "http://x").pathname);
  if (p === "/install.sh") { res.writeHead(302, { location: "https://raw.githubusercontent.com/snymrova/snyvi/main/install.sh" }); return res.end(); }
  const tries = p === "/" ? ["index.html"] : [p.slice(1), p.slice(1) + ".html"];
  const hit = tries.map(t => join(OUT, t)).find(f => f.startsWith(OUT) && existsSync(f) && statSync(f).isFile());
  const file = hit || join(OUT, "404.html");
  const body = readFileSync(file);
  const head = { "content-type": TYPES[extname(file)] || "application/octet-stream" };
  if (csp) head["content-security-policy"] = csp;
  if (extname(file) === ".mp4" && req.headers.range) {
    // "bytes=0-" is open-ended: Number("") is 0, not NaN, so test the text.
    const [a, b] = req.headers.range.replace("bytes=", "").split("-");
    const end = b === "" ? body.length - 1 : Math.min(+b, body.length - 1);
    res.writeHead(206, { ...head, "content-range": `bytes ${a}-${end}/${body.length}`, "accept-ranges": "bytes" });
    return res.end(body.subarray(+a, end + 1));
  }
  res.writeHead(hit ? 200 : 404, head);
  res.end(body);
}).listen(PORT, "127.0.0.1", () => console.log(`snyvi.com preview on http://127.0.0.1:${PORT}`));
