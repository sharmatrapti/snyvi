# snyvi.com

The product's home: a document open in snyvi. The sidebar holds the pages,
the middle reads like the window's reader view, and the rail keeps the facts
and the actions in reach. Plan: snyvi doc d9e07c9bdc. Product truth:
`PRODUCT.md`. The look is the app's (`docs/DESIGN.md`), plus Archivo, the
film's title face (`film/DESIGN.md`).

```
cargo build --bin snyvi           # the docs are rendered by `snyvi render`
node site/build.mjs --check       # → site/out; fails over the first-paint budget
node site/serve.mjs 7801          # preview the way Cloudflare serves it; never 7777
```

What it reads, so it can't drift from the app: the theme tokens from
`ui/app.css` and `ui/themes.css`; snyvi's face from `FACES` in `ui/app/`;
`docs/GUIDE.md`, `WIDGETS.md` and `PEER.md` through snyvi's own renderer;
the release titles from the tags' merge commits; the version from the
newest release. `--offline` skips GitHub and lists the releases from the tags.

| Source | Becomes |
|---|---|
| `src/pages/*.html` | `/`, `/install`, `/privacy`, the 404. A JSON comment on top says the path, title, and which row is current. `{{install}}`, `{{shot:name\|alt}}`, `{{icon:x}}`, `{{mascot:feel}}` expand. |
| `docs/*.md` | `/guide`, `/guide/widgets`, `/guide/friends` |
| the tags | `/changelog` |
| `src/_headers`, `src/_redirects` | Cloudflare's: `/install.sh` is a 302 to the script on GitHub |
| `src/og.png` | the share picture, taken from the built page at 1200×630 |

Deployed by `.github/workflows/site.yml` on pushes to `main` that touch
it, and on each release. It needs `CLOUDFLARE_API_TOKEN` (Workers Scripts
edit, Workers Routes edit and DNS edit on the snyvi.com zone) and
`CLOUDFLARE_ACCOUNT_ID` as repository secrets.
