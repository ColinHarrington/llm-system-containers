# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""Render the name-lattice mockup as an animated SVG (works as a GitHub README <img>).

A static lattice (token columns + icons + LLM span box) with the selected-path wires
flowing, occasionally toggling between the individual-token name and the LLM-span name.
Outputs two themes: name-lattice-dark.svg and name-lattice-light.svg.

Run from anywhere:  uv run mockups/name-banner/generate.py   (or: python3 generate.py)
Icon sources live alongside in ./_src.
"""
import os, base64

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "_src")
OUT = HERE

# ---- geometry ----
FS, CHARW, ICON, IGAP, PADX = 15, 9.0, 15, 6, 13
TH, TGAP, COLGAP, MARGIN = 34, 8, 26, 34
RFS = 24; RCHARW = RFS * 0.60; RCONN = 26; CARET_W = 3
RIBBON_CY_OFFSET = 30          # ribbon baseline below top margin
LAT_GAP = 30                   # gap between ribbon zone and lattice

# ---- A/B toggle timing (one loop) ----
LOOP = "12s"                   # full cycle
KT     = "0;.40;.44;.81;.85;1" # keyTimes: A holds, crossfade, B holds (~4.4s), crossfade, A
A_VALS = "1;1;0;0;1;1"
B_VALS = "0;0;1;1;0;0"

THEMES = {
  "dark":  dict(bg="#0b0e14", pill="#141a27", ink="#e7eaf1", muted="#8a93a6",
                accent="#7c9cff", line="#2a3140", tint="#7c9cff"),
  "light": dict(bg="#f5f7fa", pill="#ffffff", ink="#1b2230", muted="#5a6573",
                accent="#4361ee", line="#d7deea", tint="#4361ee"),
}

COLS = [
  [("Layered","layers"),("Lightweight","feather"),("Local","house")],
  [("Linux","tux"),("LXC","incus")],
  [("Managed","sliders-horizontal"),("Micro","shrink")],
  [("Sandbox","shield"),("Segmented","grid-2x2"),("System","server")],
  [("Compute","cpu"),("Containers","container")],
]
SEL_A = [1, 0, 0, 2, 1]                       # Lightweight Linux Managed System Containers
SPAN_WORD = "Large Language Model"


# ---- icon sources ----
def inner(path):
    s = open(path).read()
    s = s[s.index(">", s.index("<svg")) + 1:]
    return s[:s.rindex("</svg>")].strip()

LUCIDE = {k: inner(os.path.join(SRC, f"{k}.svg")) for k in
          ["layers","feather","house","sliders-horizontal","shrink","shield",
           "grid-2x2","server","cpu","container","brain-circuit"]}
WORD_ICON = {"Layered":"layers","Lightweight":"feather","Local":"house",
             "Managed":"sliders-horizontal","Micro":"shrink","Sandbox":"shield",
             "Segmented":"grid-2x2","System":"server","Compute":"cpu","Containers":"container"}
BRAND = {"Linux":"tux", "LXC":"incus"}
TUX_B64   = base64.b64encode(open(os.path.join(SRC, "tux-color.svg"), "rb").read()).decode()
INCUS_B64 = base64.b64encode(open(os.path.join(SRC, "incus.png"), "rb").read()).decode()


def content_w(word):
    return ICON + IGAP + len(word) * CHARW

def lucide_icon(key, x, y, size, color):
    s = size / 24.0
    return (f'<g transform="translate({x:.1f} {y:.1f}) scale({s:.3f})" fill="none" '
            f'stroke="{color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">'
            f'{LUCIDE[key]}</g>')

def token_icon(word, x, y, color):
    if word in BRAND:
        ref = "ic-tux" if BRAND[word] == "tux" else "ic-incus"
        return f'<use href="#{ref}" x="{x:.1f}" y="{y:.1f}"/>'
    return lucide_icon(WORD_ICON[word], x, y, ICON, color)


def build(theme):
    c = THEMES[theme]
    colw = [max(PADX*2 + content_w(w) for w, _ in col) for col in COLS]
    xs, x = [], MARGIN
    for w in colw:
        xs.append(x); x += w + COLGAP
    lat_right = xs[-1] + colw[-1]

    lat_top = MARGIN + RIBBON_CY_OFFSET + LAT_GAP
    def trow_y(r): return lat_top + r * (TH + TGAP)
    span_x = xs[0]; span_w = xs[2] + colw[2] - xs[0]
    span_y = trow_y(3)                                 # 8px below the 3-tall column
    span_bottom = span_y + TH
    lat_bottom = max(span_bottom, trow_y(3) - TGAP)

    def ribbon_w(words):
        return sum(len(w) * RCHARW for w in words) + RCONN * (len(words) - 1) + 14 + CARET_W
    rib_a = ["Lightweight","Linux","Managed","System","Containers"]
    rib_b = [SPAN_WORD, "System", "Containers"]
    CW = max(lat_right + MARGIN, ribbon_w(rib_a) + 2*MARGIN, ribbon_w(rib_b) + 2*MARGIN)
    CH = lat_bottom + MARGIN

    def tok_box(col, row):
        return (xs[col], trow_y(row), colw[col], TH)

    F = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{CW:.0f}" height="{CH:.0f}" '
         f'viewBox="0 0 {CW:.0f} {CH:.0f}" font-family="ui-monospace, SFMono-Regular, Menlo, monospace">']
    F.append('<defs>'
             f'<g id="ic-tux"><image width="{ICON}" height="{ICON}" href="data:image/svg+xml;base64,{TUX_B64}"/></g>'
             f'<g id="ic-incus"><image width="{ICON}" height="{ICON}" href="data:image/png;base64,{INCUS_B64}"/></g>'
             '</defs>')
    F.append(f'<rect width="{CW:.0f}" height="{CH:.0f}" rx="16" fill="{c["bg"]}"/>')

    # ---- base lattice (static) ----
    def draw_token(x, y, w, word):
        out = [f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{TH}" rx="9" '
               f'fill="{c["pill"]}" stroke="{c["line"]}" stroke-width="1"/>']
        cwid = content_w(word)
        cx0 = x + (w - cwid) / 2
        out.append(token_icon(word, cx0, y + (TH - ICON) / 2, c["muted"]))
        tx = cx0 + ICON + IGAP
        out.append(f'<text x="{tx:.1f}" y="{y+TH/2:.1f}" font-size="{FS}" dominant-baseline="central" '
                   f'font-weight="600"><tspan fill="{c["accent"]}">{word[0]}</tspan>'
                   f'<tspan fill="{c["muted"]}">{word[1:]}</tspan></text>')
        return "".join(out)

    for ci, col in enumerate(COLS):
        for ri, (word, _) in enumerate(col):
            bx, by, bw, _ = tok_box(ci, ri)
            F.append(draw_token(bx, by, bw, word))
    # span box (base, idle)
    F.append(f'<rect x="{span_x:.1f}" y="{span_y:.1f}" width="{span_w:.1f}" height="{TH}" rx="9" '
             f'fill="{c["pill"]}" stroke="{c["line"]}" stroke-width="1"/>')
    sp_cw = ICON + IGAP + len(SPAN_WORD) * CHARW
    sp_cx0 = span_x + (span_w - sp_cw)/2
    F.append(lucide_icon("brain-circuit", sp_cx0, span_y + (TH-19)/2, 19, c["accent"]))
    sp_tx = sp_cx0 + ICON + IGAP
    spans = "".join(f'<tspan fill="{c["accent"]}">{w[0]}</tspan><tspan fill="{c["muted"]}">{w[1:]}</tspan> '
                    for w in SPAN_WORD.split(" "))
    F.append(f'<text x="{sp_tx:.1f}" y="{span_y+TH/2:.1f}" font-size="{FS}" dominant-baseline="central" '
             f'font-weight="600">{spans}</text>')

    # ---- overlay helpers ----
    def ring(x, y, w):
        return (f'<rect x="{x-1:.1f}" y="{y-1:.1f}" width="{w+2:.1f}" height="{TH+2}" rx="10" '
                f'fill="{c["tint"]}" fill-opacity="0.14" stroke="{c["accent"]}" stroke-width="1.6"/>')
    def dim(x, y, w):
        return f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{TH}" rx="9" fill="{c["bg"]}" opacity="0.62"/>'
    def wire(p1, p2):
        x1, y1 = p1; x2, y2 = p2; dx = (x2 - x1) * 0.5
        return (f'<path d="M {x1:.1f} {y1:.1f} C {x1+dx:.1f} {y1:.1f}, {x2-dx:.1f} {y2:.1f}, {x2:.1f} {y2:.1f}" '
                f'fill="none" stroke="{c["accent"]}" stroke-width="2" stroke-linecap="round" '
                f'stroke-dasharray="6 7"><animate attributeName="stroke-dashoffset" values="0;-26" '
                f'dur="1.1s" repeatCount="indefinite"/></path>')
    def node(x, y):
        return f'<circle cx="{x:.1f}" cy="{y:.1f}" r="3" fill="{c["accent"]}"/>'
    def ribbon(words, cy):
        total = ribbon_w(words); sx = (CW - total) / 2
        frags, cx, edges = [], sx, []
        for w in words:
            frags.append(f'<text x="{cx:.1f}" y="{cy:.1f}" font-size="{RFS}" font-weight="600" '
                         f'dominant-baseline="central"><tspan fill="{c["accent"]}">{w[0]}</tspan>'
                         f'<tspan fill="{c["ink"]}">{w[1:]}</tspan></text>')
            edges.append((cx, cx + len(w) * RCHARW)); cx += len(w) * RCHARW + RCONN
        for i in range(len(edges) - 1):
            x1 = edges[i][1] + 6; x2 = edges[i+1][0] - 6
            frags.append(f'<line x1="{x1:.1f}" y1="{cy:.1f}" x2="{x2:.1f}" y2="{cy:.1f}" '
                         f'stroke="{c["accent"]}" stroke-width="2" stroke-linecap="round" '
                         f'stroke-dasharray="5 6" opacity="0.8"><animate attributeName="stroke-dashoffset" '
                         f'values="0;-22" dur="1.1s" repeatCount="indefinite"/></line>')
        cxp = edges[-1][1] + 10
        frags.append(f'<rect x="{cxp:.1f}" y="{cy-RFS*0.6:.1f}" width="{CARET_W}" height="{RFS*1.2:.1f}" '
                     f'rx="1.5" fill="{c["accent"]}"><animate attributeName="opacity" values="1;1;0;0" '
                     f'keyTimes="0;.5;.5;1" dur="1.05s" repeatCount="indefinite"/></rect>')
        return "".join(frags)

    rib_cy = MARGIN + RIBBON_CY_OFFSET - 8

    # ---- state A: individual-token path ----
    A = [f'<g opacity="1"><animate attributeName="opacity" values="{A_VALS}" '
         f'keyTimes="{KT}" dur="{LOOP}" repeatCount="indefinite"/>']
    selpts = []
    for ci in range(5):
        bx, by, bw, _ = tok_box(ci, SEL_A[ci])
        A.append(ring(bx, by, bw))
        selpts.append(((bx + bw, by + TH/2), (bx, by + TH/2)))
    for i in range(4):
        rp, lp = selpts[i][0], selpts[i+1][1]
        A.append(wire(rp, lp)); A.append(node(*rp)); A.append(node(*lp))
    A.append(ribbon(rib_a, rib_cy)); A.append('</g>')
    F.append("".join(A))

    # ---- state B: LLM span path ----
    B = [f'<g opacity="0"><animate attributeName="opacity" values="{B_VALS}" '
         f'keyTimes="{KT}" dur="{LOOP}" repeatCount="indefinite"/>']
    for ci in (0, 1, 2):
        for ri in range(len(COLS[ci])):
            bx, by, bw, _ = tok_box(ci, ri)
            B.append(dim(bx, by, bw))
    B.append(ring(span_x, span_y, span_w))
    sysb, conb = tok_box(3, 2), tok_box(4, 1)
    B.append(ring(*sysb[:3])); B.append(ring(*conb[:3]))
    sp_r = (span_x + span_w, span_y + TH/2)
    sys_l = (sysb[0], sysb[1] + TH/2); sys_r = (sysb[0] + sysb[2], sysb[1] + TH/2)
    con_l = (conb[0], conb[1] + TH/2)
    for rp, lp in [(sp_r, sys_l), (sys_r, con_l)]:
        B.append(wire(rp, lp)); B.append(node(*rp)); B.append(node(*lp))
    B.append(ribbon(rib_b, rib_cy)); B.append('</g>')
    F.append("".join(B))

    F.append("</svg>")
    return "\n".join(F)


if __name__ == "__main__":
    for t in THEMES:
        path = os.path.join(OUT, f"name-lattice-{t}.svg")
        open(path, "w").write(build(t))
        print("wrote", os.path.relpath(path, os.getcwd()) if os.getcwd() in path else path)
