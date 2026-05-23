"""Render tiles as ASCII art for identification."""
import os
from PIL import Image

tiles_dir = "assets/kenney_tiny-town/Tiles"
cols, rows = 12, 11

# ASCII palette based on dominant hue
def pixel_char(r, g, b, a):
    if a < 128:
        return " "
    if g > 150 and g > r + 20 and g > b + 20:
        return "g"  # green
    if r > 200 and g < 120 and b < 120:
        return "R"  # red
    if r > 180 and g > 150 and b < 120:
        return "y"  # yellow/tan
    if b > 150 and b > r + 10 and b > g + 10:
        return "b"  # blue
    if r > 200 and g > 200 and b > 200:
        return "."  # white
    if r > 150 and g < 130 and b < 130 and r - g > 30:
        return "s"  # skin
    if r + g + b < 120:
        return "#"  # dark
    if r > 100 and g > 60 and b < 80:
        return "w"  # brown/wood
    return "@"  # other

print(f"{'idx':<5} {'image (4x scaled ASCII)':<70} {'desc'}")
print("=" * 120)

for idx in range(132):
    path = os.path.join(tiles_dir, f"tile_{idx:04d}.png")
    if not os.path.exists(path):
        continue
    img = Image.open(path).convert("RGBA")

    # Build 8x8 ASCII (2x downsampled)
    lines = []
    for y in range(0, 16, 2):
        line = ""
        for x in range(0, 16, 2):
            r, g, b, a = 0, 0, 0, 0
            count = 0
            for dy in range(2):
                for dx in range(2):
                    pr, pg, pb, pa = img.getpixel((x + dx, y + dy))
                    r += pr; g += pg; b += pb; a += pa; count += 1
            r //= count; g //= count; b //= count; a //= count
            line += pixel_char(r, g, b, a)
        lines.append(line)

    # Simple description based on content
    pixels = list(img.getdata())
    total = sum(1 for p in pixels if p[3] > 128)
    greens = sum(1 for p in pixels if p[3] > 128 and p[1] > 130 and p[1] > p[0] + 15 and p[1] > p[2] + 15)
    reds = sum(1 for p in pixels if p[3] > 128 and p[0] > 180 and p[1] < 100 and p[2] < 100)
    skins = sum(1 for p in pixels if p[3] > 128 and p[0] > 150 and p[1] < 140 and p[2] < 120 and p[0] - p[1] > 20)
    blues = sum(1 for p in pixels if p[3] > 128 and p[2] > 140 and p[2] > p[0] + 10 and p[2] > p[1] + 10)
    whites = sum(1 for p in pixels if p[3] > 128 and p[0] > 200 and p[1] > 200 and p[2] > 200)
    darks = sum(1 for p in pixels if p[3] > 128 and p[0] < 80 and p[1] < 80 and p[2] < 80)

    if total == 0:
        desc = "empty"
    elif greens > 200:
        desc = "GRASS/solid green"
    elif greens > 100:
        desc = "VEGETATION"
    elif skins > 100:
        desc = "CHARACTER"
    elif reds > 80:
        desc = "RED ROOF"
    elif blues > 150:
        desc = "WATER/BLUE"
    elif darks > 150:
        desc = "DARK"
    elif total < 50:
        desc = "sparse"
    else:
        desc = f"Mixed(g={greens} r={reds} s={skins} b={blues} d={darks})"

    img_str = "  ".join(lines)
    print(f"{idx:<5} {img_str:<70} {desc}")
