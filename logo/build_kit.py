"""Build the crablink logo kit masters (all geometry constructed, no fonts)."""
from math import cos, sin, radians, pi
from pathlib import Path

OUT = Path(__file__).parent / "masters"
OUT.mkdir(exist_ok=True)

RED = "#E0412A"     # Crab Red
INK = "#17181C"     # Shell Ink
WHITE = "#FFFFFF"


def f(v):
    return f"{v:.2f}".rstrip("0").rstrip(".")


def P(c, r, a):
    return (c[0] + r * cos(radians(a)), c[1] + r * sin(radians(a)))


def pt(p):
    return f"{f(p[0])} {f(p[1])}"


# ---- primitives (all outer contours clockwise → nonzero fill = union) ------
def rect(x, y, w, h):
    return f"M {f(x)} {f(y)} H {f(x + w)} V {f(y + h)} H {f(x)} Z"


def circle(c, r):
    a, b = (c[0] + r, c[1]), (c[0] - r, c[1])
    return f"M {pt(a)} A {f(r)} {f(r)} 0 1 1 {pt(b)} A {f(r)} {f(r)} 0 1 1 {pt(a)} Z"


def circle_ccw(c, r):
    a, b = (c[0] + r, c[1]), (c[0] - r, c[1])
    return f"M {pt(a)} A {f(r)} {f(r)} 0 1 0 {pt(b)} A {f(r)} {f(r)} 0 1 0 {pt(a)} Z"


def ring(c, R, r):
    return circle(c, R) + " " + circle_ccw(c, r)


def ring_arc(c, R, r, a0, a1):
    """Band of a ring from angle a0 clockwise to a1 (degrees, y down), radial ends."""
    span = (a1 - a0) % 360
    big = 1 if span > 180 else 0
    return (f"M {pt(P(c, R, a0))} A {f(R)} {f(R)} 0 {big} 1 {pt(P(c, R, a1))} "
            f"L {pt(P(c, r, a1))} A {f(r)} {f(r)} 0 {big} 0 {pt(P(c, r, a0))} Z")


def poly(pts):
    area = sum(x0 * y1 - x1 * y0 for (x0, y0), (x1, y1) in zip(pts, pts[1:] + pts[:1]))
    if area < 0:          # y-down: positive shoelace = clockwise on screen
        pts = pts[::-1]
    return "M " + " L ".join(pt(p) for p in pts) + " Z"


def pac(c, r, facing, opening=60):
    a0, a1 = facing + opening / 2, facing - opening / 2 + 360
    return (f"M {pt(c)} L {pt(P(c, r, a0))} A {f(r)} {f(r)} 0 1 1 {pt(P(c, r, a1))} Z")


# ---- symbol ---------------------------------------------------------------
def shell(x0, x1, y0, y1, t):
    ro = (y1 - y0) / 2
    ri = ro - t
    return (f"M {f(x0 + ro)} {f(y0)} H {f(x1 - ro)} A {f(ro)} {f(ro)} 0 0 1 {f(x1 - ro)} {f(y1)} "
            f"H {f(x0 + ro)} A {f(ro)} {f(ro)} 0 0 1 {f(x0 + ro)} {f(y0)} Z "
            f"M {f(x0 + ro)} {f(y0 + t)} A {f(ri)} {f(ri)} 0 0 0 {f(x0 + ro)} {f(y1 - t)} "
            f"H {f(x1 - ro)} A {f(ri)} {f(ri)} 0 0 0 {f(x1 - ro)} {f(y0 + t)} Z")


def symbol_d():
    """Full mark: chain-link shell, two pincers, '=' legs. 256 canvas."""
    legs = " ".join(rect(x, y, 22, 12) for x in (22, 212) for y in (152, 176))
    return " ".join([shell(52, 204, 114, 202, 28),
                     pac((56, 86), 32, -120), pac((200, 86), 32, -60), legs])


# ---- wordmark: geometric monoline, x-height 100, stroke 22 ------------------
S = 22
R = 51.5            # round letters overshoot x-height by 1.5 top and bottom
r_in = R - S
ASC = -50


def g_c(x):
    return ring_arc((x + R, 50), R, r_in, 30, 330), 2 * R


def g_r(x):
    return rect(x, 0, S, 100) + " " + ring_arc((x + R, 50), R, r_in, 180, 300), R + R * cos(radians(60))


def g_a(x):
    return ring((x + R, 50), R, r_in) + " " + rect(x + 2 * R - S, -1.5, S, 101.5), 2 * R


def g_b(x):
    return ring((x + R, 50), R, r_in) + " " + rect(x, ASC, S, 151.5), 2 * R


def g_l(x):
    return rect(x, ASC, S, 150), S


def g_i(x):
    return rect(x, 0, S, 100) + " " + circle((x + S / 2, -29), 13.5), S


def g_n(x):
    return (rect(x, 0, S, 100) + " " + ring_arc((x + R, 50), R, r_in, 180, 360) + " "
            + rect(x + 2 * R - S, 50, S, 50)), 2 * R


def g_k(x):
    w = S * 2 ** 0.5                       # horizontal width of a 45° band
    arm = poly([(x + 62, 0), (x + 62 + w, 0), (x + S, 62 + w - S), (x + S, 62 - S)])
    leg = poly([(x + 62, 100), (x + 62 + w, 100), (x + 27.6, 34.4), (x + 12, 50)])
    return rect(x, ASC, S, 150) + " " + arm + " " + leg, 62 + w


# optical spacing: gap before each glyph (round/open sides tighter)
GLYPHS = [(g_c, 0), (g_r, 12), (g_a, 6), (g_b, 20), (g_l, 16), (g_i, 20), (g_n, 20), (g_k, 20)]


def wordmark():
    parts, x = [], 0
    for g, gap in GLYPHS:
        x += gap
        d, w = g(x)
        parts.append(d)
        x += w
    return " ".join(parts), x


# ---- files ----------------------------------------------------------------
def svg(w, h, body, title="crablink logo"):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {f(w)} {f(h)}" '
            f'width="{f(w)}" height="{f(h)}"><title>{title}</title>\n{body}\n</svg>\n')


def save(name, w, h, body):
    (OUT / name).write_text(svg(w, h, body))


WM, WM_W = wordmark()
WM_H = 151.5 + 1.5          # ascender top -50 .. baseline overshoot 101.5


def wm_group(x, y, s, color):
    # (x, y) = position of x-height top-left; s = scale
    return f'<path fill="{color}" transform="translate({f(x)} {f(y)}) scale({s})" d="{WM}"/>'


def build():
    sym = symbol_d()
    save("crablink-symbol.svg", 256, 256, f'<path fill="{RED}" d="{sym}"/>')
    save("crablink-wordmark.svg", WM_W, WM_H,
         f'<path fill="{INK}" transform="translate(0 50)" d="{WM}"/>')

    # horizontal: symbol visual box y 54..202; x-height centred on the shell (114..202)
    s = 0.8
    xh = 100 * s
    y_top = 158 - xh / 2
    x_wm = 256 + 4
    w = x_wm + WM_W * s + 22
    for name, wc in (("crablink-horizontal.svg", INK), ("crablink-horizontal-reversed.svg", WHITE)):
        save(name, w, 256, f'<path fill="{RED}" d="{sym}"/>\n{wm_group(x_wm, y_top, s, wc)}')

    # stacked: symbol on top, wordmark centred below
    s2 = 0.5
    ww = WM_W * s2
    W = max(256, ww + 48)
    sx = (W - 256) / 2
    y_wm = 230 + 50 * s2          # ascender top sits at 230
    H = y_wm + 101.5 * s2 + 40
    for name, wc in (("crablink-stacked.svg", INK), ("crablink-stacked-reversed.svg", WHITE)):
        save(name, W, H, f'<path fill="{RED}" transform="translate({f(sx)} 0)" d="{sym}"/>\n'
                         f'{wm_group((W - ww) / 2, y_wm, s2, wc)}')


def contrast(a, b):
    def lum(h):
        c = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
        c = [v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4 for v in c]
        return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


if __name__ == "__main__":
    build()
    for a, b in ((RED, WHITE), (RED, INK), (INK, WHITE)):
        print(f"contrast {a} vs {b}: {contrast(a, b):.2f}")
    print("wordmark width", f(WM_W))
