# name-banner

Animated SVG of the name lattice, sized for a GitHub README `<img>` (pure SMIL — no
JS/CSS — so it animates inside the sanitized README sandbox). The interactive original
lives in [`../name-tokens-llm-span/`](../name-tokens-llm-span/).

The wires between tokens flow continuously, and the selected path occasionally toggles
between the individual-token name (*Lightweight Linux Managed System Containers*) and the
collapsed LLM-span form (*Large Language Model System Containers*).

## Files

| File | What |
|------|------|
| `generate.py` | builds the SVGs (stdlib only; `uv run generate.py` or `python3 generate.py`) |
| `_src/` | icon sources the generator embeds — Lucide line icons + Tux/Incus brand marks |
| `name-lattice-dark.svg` / `name-lattice-light.svg` | the generated, self-contained outputs |
| `README-snippet.md` | paste-ready `<picture>` block (auto-swaps dark/light to the viewer's theme) |
| `preview.html` | view both themes locally as a browser would render them |

## Tuning

Knobs live at the top of `generate.py`: geometry (`FS`, `TH`, `COLGAP`, …), the
`THEMES` palettes, the word/icon columns (`COLS`, `SEL_A`), and the A/B toggle timing
(`LOOP`, `KT`). Edit, re-run, done.
