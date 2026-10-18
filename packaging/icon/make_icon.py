"""Draws the POS Heart app icon (graphite tile, saffron heart, paper receipt
edge) and writes every packaged size. Run from the repository root:

    python3 packaging/icon/make_icon.py

Outputs: packaging/icon/icon-1024.png, AppIcon.icns (macOS, needs iconutil),
icon.ico (Windows) and apps/pos-desktop/assets/icon-256.png (window icon).
Colours come from the design tokens (graphite canvas, saffron accent).
"""
import math, os, shutil, subprocess, sys
from PIL import Image, ImageDraw, ImageFilter

GRAPHITE_TOP = (52, 54, 59)
GRAPHITE_BOTTOM = (31, 32, 35)
SAFFRON = (240, 178, 50)
SAFFRON_DARK = (214, 150, 28)
PAPER = (244, 243, 239)
S = 4  # supersampling
N = 1024 * S

def heart_mask(size, scale, cx, cy):
    """Classic parametric heart, filled, as an L mask."""
    pts = []
    for i in range(720):
        t = math.tau * i / 720
        x = 16 * math.sin(t) ** 3
        y = 13 * math.cos(t) - 5 * math.cos(2 * t) - 2 * math.cos(3 * t) - math.cos(4 * t)
        pts.append((cx + x * scale, cy - y * scale))
    m = Image.new("L", (size, size), 0)
    ImageDraw.Draw(m).polygon(pts, fill=255)
    return m

def draw():
    img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    # macOS grid: 824 px body inside a 1024 canvas, ~185 px corner radius.
    inset, radius = 100 * S, 185 * S
    body = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    grad = Image.new("RGBA", (N, N))
    gd = ImageDraw.Draw(grad)
    for y in range(N):
        k = y / N
        c = tuple(round(a + (b - a) * k) for a, b in zip(GRAPHITE_TOP, GRAPHITE_BOTTOM))
        gd.line([(0, y), (N, y)], fill=c + (255,))
    mask = Image.new("L", (N, N), 0)
    ImageDraw.Draw(mask).rounded_rectangle([inset, inset, N - inset, N - inset], radius, fill=255)
    body.paste(grad, (0, 0), mask)
    # Soft shadow under the tile.
    shadow = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle([inset, inset + 14 * S, N - inset, N - inset + 14 * S], radius, fill=(0, 0, 0, 90))
    shadow = shadow.filter(ImageFilter.GaussianBlur(18 * S))
    img = Image.alpha_composite(img, shadow)
    img = Image.alpha_composite(img, body)
    # Paper receipt peeking from the bottom edge, with a zigzag tear.
    paper = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    pd = ImageDraw.Draw(paper)
    left, right, top = 330 * S, 694 * S, 600 * S
    bottom = N  # runs out of the tile edge (clipped below)
    teeth = 9
    zig = [(left, top)]
    for i in range(teeth * 2 + 1):
        x = left + (right - left) * i / (teeth * 2)
        zig.append((x, top - (14 * S if i % 2 else 0)))
    zig += [(right, bottom), (left, bottom)]
    pd.polygon(zig, fill=PAPER + (255,))
    for j, w in enumerate((0.62, 0.48, 0.55)):
        y = (676 + j * 40) * S
        pd.rounded_rectangle([left + 44 * S, y, left + 44 * S + (right - left - 88 * S) * w, y + 14 * S], 7 * S, fill=(201, 199, 191, 255))
    clip = Image.new("L", (N, N), 0)
    ImageDraw.Draw(clip).rounded_rectangle([inset, inset, N - inset, N - inset], radius, fill=255)
    paper.putalpha(Image.composite(paper.getchannel("A"), Image.new("L", (N, N), 0), clip))
    img = Image.alpha_composite(img, paper)
    # Saffron heart above the receipt, lit from the top.
    hm = heart_mask(N, 15.5 * S, N / 2, 438 * S)
    heart_grad = Image.new("RGBA", (N, N))
    hd = ImageDraw.Draw(heart_grad)
    for y in range(N):
        k = min(1, max(0, (y - 220 * S) / (440 * S)))
        c = tuple(round(a + (b - a) * k) for a, b in zip(SAFFRON, SAFFRON_DARK))
        hd.line([(0, y), (N, y)], fill=c + (255,))
    glow = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    glow.paste((0, 0, 0, 110), (0, 0), hm.filter(ImageFilter.GaussianBlur(10 * S)).point(lambda v: v))
    glow = glow.transform(glow.size, Image.AFFINE, (1, 0, 0, 0, 1, -10 * S))
    img = Image.alpha_composite(img, glow)
    heart = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    heart.paste(heart_grad, (0, 0), hm)
    img = Image.alpha_composite(img, heart)
    return img.resize((1024, 1024), Image.LANCZOS)

def main():
    root = os.getcwd()
    out = os.path.join(root, "packaging", "icon")
    master = draw()
    master.save(os.path.join(out, "icon-1024.png"))
    assets = os.path.join(root, "apps", "pos-desktop", "assets")
    os.makedirs(assets, exist_ok=True)
    master.resize((256, 256), Image.LANCZOS).save(os.path.join(assets, "icon-256.png"))
    master.save(os.path.join(out, "icon.ico"), sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    iconset = os.path.join(out, "AppIcon.iconset")
    shutil.rmtree(iconset, ignore_errors=True)
    os.makedirs(iconset)
    for size in (16, 32, 128, 256, 512):
        master.resize((size, size), Image.LANCZOS).save(os.path.join(iconset, f"icon_{size}x{size}.png"))
        master.resize((size * 2, size * 2), Image.LANCZOS).save(os.path.join(iconset, f"icon_{size}x{size}@2x.png"))
    if shutil.which("iconutil"):
        subprocess.run(["iconutil", "-c", "icns", iconset, "-o", os.path.join(out, "AppIcon.icns")], check=True)
    shutil.rmtree(iconset, ignore_errors=True)

if __name__ == "__main__":
    sys.exit(main())
