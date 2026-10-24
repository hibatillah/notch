---
paths:
  - "src/pages/**"
  - "src/host/**"
  - "src/page.rs"
  - "src/gfx/**"
  - "src/ui/**"
---

# Pages and host boundaries

Read `docs/adding-a-page.md` before writing a page and `docs/architecture.md` before restructuring.

- A page lives in `src/pages/<name>/` and is listed only in `src/pages/mod.rs`; adding or removing one touches no
  other file.
- A page depends on `page`, `gfx` and `ui` only.
- `host`, `gfx`, `ui` and `page` stay ignorant of any particular page.
- Page timer ids are `100 + page_index * 16 + n`, with `n` below 16.
- Page settings keys start with the page id (`media.lyrics`).
- Pages draw through `Canvas` and multiply colors by the `opacity` they are given, so content follows the open and
  close animation.
