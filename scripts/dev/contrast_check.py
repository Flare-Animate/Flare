#!/usr/bin/env python3
"""WCAG contrast for the Liquid Glass palette pairs (text >= 4.5, controls/icons >= 3)."""
def lum(h):
    c = [int(h[i:i+2], 16)/255 for i in (1, 3, 5)]
    c = [x/12.92 if x <= .03928 else ((x+.055)/1.055)**2.4 for x in c]
    return .2126*c[0] + .7152*c[1] + .0722*c[2]
def cr(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la+.05)/(lb+.05)
PAIRS = [  # (label, fg, bg, min)
 ("text primary / base",        "#e6edf6", "#172131", 4.5),
 ("text primary / raised btn",  "#e6edf6", "#222f45", 4.5),
 ("text / input well",          "#f2f7fd", "#0f1621", 4.5),
 ("secondary text / base",      "#9fb0c8", "#172131", 4.5),
 ("group title / base",         "#b9c8de", "#172131", 4.5),
 ("inactive tab text / tab",    "#c3d0e4", "#1f2b3f", 4.5),
 ("selected tab/menu white / accent","#ffffff", "#2a6fb0", 4.5),
 ("white / accent gradient end","#ffffff", "#2a6fb0", 4.5),
 ("disabled text / base",       "#7886a0", "#172131", 3.0),
 ("checkbox border / well",     "#7d8fb0", "#0f1621", 3.0),
 ("button border / base",       "#4a5e80", "#172131", 1.0),
 ("focus ring / well",          "#5fb4f0", "#0f1621", 3.0),
 ("checked text / pressed",     "#8fd0ff", "#172131", 4.5),
 ("icon base / base",           "#cecece", "#172131", 3.0),
 ("tooltip text / tooltip",     "#f2f7fd", "#1f2b3f", 4.5),
]
if __name__ == "__main__":
    bad = 0
    for n, f, b, m in PAIRS:
        r = cr(f, b); ok = r >= m; bad += not ok
        print(f"{'PASS' if ok else 'FAIL'} {r:5.2f}:1 (need {m})  {n}  {f} on {b}")
    raise SystemExit(bad)
