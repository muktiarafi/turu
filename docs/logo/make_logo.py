"""Turu logo: a sleeping bot. Rounded tile, two closed eyes (the u's of "turu") with lashes."""
import math
VIOLET="#7C6CF0"; NIGHT="#11131A"; LIGHT="#E8EAF2"; WHITE="#FFFFFF"
def f(x): return f"{x:.2f}".rstrip('0').rstrip('.')
def pt(p): return f"{f(p[0])} {f(p[1])}"
def u(a): t=math.radians(a); return (math.cos(t), math.sin(t))
def add(*vs): return (sum(v[0] for v in vs), sum(v[1] for v in vs))
def mul(v,k): return (v[0]*k, v[1]*k)

def eye(cx, cy, r=30, w=18, lashes=(120, 90, 60), lw=12, reach=18):
    """One closed contour: lower lid (half ring, round caps) with lashes grown out of its outer edge."""
    c=(cx,cy); Ro=r+w/2; Ri=r-w/2; h=lw/2
    base=math.sqrt(Ro*Ro-h*h); tip=Ro+reach
    d=f"M{pt((cx-Ro,cy))}"
    for a in sorted(lashes, reverse=True):          # walk the outer edge from left (180°) to right (0°)
        n=u(a+90); dd=math.degrees(math.asin(h/Ro))
        d+=f" A{f(Ro)} {f(Ro)} 0 0 0 {pt(add(c,mul(u(a+dd),Ro)))}"
        d+=f" L{pt(add(c,mul(u(a),tip),mul(n,h)))}"
        d+=f" A{f(h)} {f(h)} 0 0 0 {pt(add(c,mul(u(a),tip),mul(n,-h)))}"
        d+=f" L{pt(add(c,mul(u(a-dd),Ro)))}"
    d+=f" A{f(Ro)} {f(Ro)} 0 0 0 {pt((cx+Ro,cy))}"
    d+=f" A{f(w/2)} {f(w/2)} 0 0 0 {pt((cx+Ri,cy))}"
    d+=f" A{f(Ri)} {f(Ri)} 0 0 1 {pt((cx-Ri,cy))}"
    d+=f" A{f(w/2)} {f(w/2)} 0 0 0 {pt((cx-Ro,cy))} Z"
    return d

def tile(x, y, s, rr):
    """Rounded square with smoothed (continuous-curvature-ish) corners."""
    k=rr*1.28; hnd=rr*0.64
    X,Y=x+s,y+s
    return (f"M{f(x+k)} {f(y)} H{f(X-k)} C{f(X-k+hnd)} {f(y)} {f(X)} {f(y+k-hnd)} {f(X)} {f(y+k)}"
            f" V{f(Y-k)} C{f(X)} {f(Y-k+hnd)} {f(X-k+hnd)} {f(Y)} {f(X-k)} {f(Y)}"
            f" H{f(x+k)} C{f(x+k-hnd)} {f(Y)} {f(x)} {f(Y-k+hnd)} {f(x)} {f(Y-k)}"
            f" V{f(y+k)} C{f(x)} {f(y+k-hnd)} {f(x+k-hnd)} {f(y)} {f(x+k)} {f(y)} Z")

def eyes(small=False):
    if small:  # 16-32 px cut: no lashes, heavier lids, eyes a bit lower
        return eye(80,112,32,28,lashes=()), eye(176,112,32,28,lashes=())
    return eye(78,104,34,20,lw=13), eye(178,104,34,20,lw=13)

def symbol(tile_fill, eye_fill, small=False, pad=16):
    s=256-2*pad; t=tile(pad,pad,s,s*0.2)
    e=" ".join(eyes(small))
    if eye_fill is None:   # one colour: eyes are holes in the tile
        return f'<path fill="{tile_fill}" fill-rule="evenodd" d="{t} {e}"/>'
    return f'<path fill="{tile_fill}" d="{t}"/><path fill="{eye_fill}" d="{e}"/>'

def wordmark(col, sw=24, x=0):
    st=f'fill="none" stroke="{col}" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"'
    p=[f'<path d="M{x+20} 76 V176 A32 32 0 0 0 {x+52} 208 H{x+60} M{x} 120 H{x+56}" {st}/>']
    x+=88
    for _ in range(2):
        if _==1: x+=96
        p.append(f'<path d="M{x} 120 V160 A48 48 0 0 0 {x+96} 160 V120 M{x+96} 160 V208" {st}/>')
        if _==0:
            x+=136; p.append(f'<path d="M{x} 208 V120 M{x} 168 A48 48 0 0 1 {x+48} 120 H{x+56}" {st}/>')
    return "".join(p), x+96+sw/2   # width incl. half stroke

def svg(body, w, h, title="Turu", y=0):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 {f(y)} {f(w)} {f(h)}" width="{f(w)}" height="{f(h)}" '
            f'role="img" aria-label="{title}"><title>{title}</title>{body}</svg>\n')

def lockup(word_col, eye_fill=WHITE, tile_fill=VIOLET):
    # symbol tile sized to the wordmark: top at the t's ascender, bottom on the baseline (+ overshoot)
    S=164; y0=58; sc=S/256*256/224    # tile fills the scaled box minus its own padding
    sym=f'<g transform="translate({f(-16*sc)} {f(y0-16*sc)}) scale({f(sc)})">{symbol(tile_fill,eye_fill)}</g>'
    wm,ww=wordmark(word_col, x=S+40)
    return svg(sym+wm, ww+6, 188, "Turu", y=46)   # cropped to the art plus a small margin

import os, sys
out=sys.argv[1] if len(sys.argv)>1 else os.path.dirname(os.path.abspath(__file__)); os.makedirs(out, exist_ok=True)
w=lambda n,s: open(os.path.join(out,n),"w").write(s)
w("turu-symbol.svg", svg(symbol(VIOLET,WHITE),256,256))
w("turu-symbol-small.svg", svg(symbol(VIOLET,WHITE,small=True),256,256))
w("turu-symbol-mono.svg", svg(symbol("#000000",None),256,256))
def macos(body):  # Apple's icon grid: the tile is 824/1024 of the canvas
    k=256*824/1024/224
    return svg(f'<g transform="translate({f(128-128*k)} {f(128-128*k)}) scale({f(k)})">{body}</g>',256,256)
w("turu-symbol-macos.svg", macos(symbol(VIOLET,WHITE)))
w("turu-symbol-macos-small.svg", macos(symbol(VIOLET,WHITE,small=True)))
w("turu-logo-light.svg", lockup(NIGHT))   # for light backgrounds
w("turu-logo-dark.svg", lockup(LIGHT))    # for dark backgrounds
