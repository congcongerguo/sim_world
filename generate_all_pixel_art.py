"""
Generate ALL pixel art procedurally at 32x32 - clean, readable pixel art for every game element.
Replaces SDXL-generated sprites that looked like noisy garbage when downscaled.
"""
import os
import math
import struct
from PIL import Image

OUTPUT_DIR = "D:/work/rust_len/sim_world/assets/pixel_prototypes"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def new_img(bg=(0, 0, 0, 0)):
    return Image.new("RGBA", (32, 32), bg)


def px_set(img, x, y, color):
    """Set pixel if in bounds."""
    if 0 <= x < 32 and 0 <= y < 32:
        img.putpixel((x, y), color)


def rect(img, x, y, w, h, color):
    """Fill a rectangle."""
    for dy in range(h):
        for dx in range(w):
            px_set(img, x + dx, y + dy, color)


def circle(img, cx, cy, r, color):
    """Fill a circle using midpoint algorithm."""
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            if dx * dx + dy * dy <= r * r:
                px_set(img, cx + dx, cy + dy, color)


def tri(img, x1, y1, x2, y2, x3, y3, color):
    """Fill a triangle (coarse)."""
    min_x = max(0, min(x1, x2, x3))
    max_x = min(31, max(x1, x2, x3))
    min_y = max(0, min(y1, y2, y3))
    max_y = min(31, max(y1, y2, y3))
    for y in range(min_y, max_y + 1):
        for x in range(min_x, max_x + 1):
            # Barycentric check
            def sign(ax, ay, bx, by, cx, cy):
                return (ax - cx) * (by - cy) - (bx - cx) * (ay - cy)
            d1 = sign(x, y, x1, y1, x2, y2)
            d2 = sign(x, y, x2, y2, x3, y3)
            d3 = sign(x, y, x3, y3, x1, y1)
            has_neg = (d1 < 0) or (d2 < 0) or (d3 < 0)
            has_pos = (d1 > 0) or (d2 > 0) or (d3 > 0)
            if not (has_neg and has_pos):
                px_set(img, x, y, color)


def hline(img, x1, x2, y, color):
    for x in range(x1, x2 + 1):
        px_set(img, x, y, color)


def vline(img, y1, y2, x, color):
    for y in range(y1, y2 + 1):
        px_set(img, x, y, color)


def outline(img, color):
    """Draw a 1px border around the image."""
    for x in range(32):
        px_set(img, x, 0, color)
        px_set(img, x, 31, color)
    for y in range(32):
        px_set(img, 0, y, color)
        px_set(img, 31, y, color)


# ===========================================================================
# VEGETATION (7)
# ===========================================================================

def gen_deciduous_tree():
    """Deciduous tree - brown trunk + green round canopy."""
    img = new_img()
    # Trunk
    rect(img, 14, 18, 4, 12, (101, 67, 33, 255))
    # Canopy - big circle of green with highlights
    circle(img, 16, 12, 10, (56, 142, 60, 255))
    circle(img, 16, 12, 7, (76, 175, 80, 255))
    circle(img, 14, 10, 5, (129, 199, 132, 255))
    # Highlights
    px_set(img, 12, 8, (200, 230, 200, 255))
    px_set(img, 18, 7, (200, 230, 200, 255))
    return img


def gen_pine_tree():
    """Pine tree - triangle shape with dark green."""
    img = new_img()
    # Trunk
    rect(img, 14, 22, 4, 8, (101, 67, 33, 255))
    # Layers of triangles
    tri(img, 16, 2, 4, 22, 28, 22, (27, 94, 32, 255))
    tri(img, 16, 6, 6, 24, 26, 24, (46, 125, 50, 255))
    tri(img, 16, 12, 8, 26, 24, 26, (76, 160, 80, 255))
    # Snow on top
    px_set(img, 16, 3, (200, 230, 210, 255))
    px_set(img, 15, 4, (200, 230, 210, 255))
    px_set(img, 17, 4, (200, 230, 210, 255))
    return img


def gen_palm_tree():
    """Palm tree - curved trunk with fronds."""
    img = new_img()
    # Trunk
    rect(img, 15, 16, 3, 14, (121, 85, 72, 255))
    px_set(img, 14, 18, (121, 85, 72, 255))
    px_set(img, 16, 28, (121, 85, 72, 255))
    # Fronds
    for angle in range(0, 360, 45):
        import math as m
        for r in range(1, 9):
            x = 16 + int(m.cos(m.radians(angle)) * r)
            y = 14 + int(m.sin(m.radians(angle)) * r * 0.6)
            px_set(img, x, y, (46, 125, 50, 255))
    # Coconuts
    px_set(img, 15, 15, (121, 85, 72, 255))
    px_set(img, 17, 15, (121, 85, 72, 255))
    px_set(img, 16, 16, (101, 67, 33, 255))
    return img


def gen_bush():
    """Small round bush."""
    img = new_img()
    circle(img, 16, 20, 8, (56, 142, 60, 255))
    circle(img, 16, 20, 6, (76, 175, 80, 255))
    circle(img, 14, 18, 4, (129, 199, 132, 255))
    # Berries
    px_set(img, 13, 20, (244, 67, 54, 255))
    px_set(img, 18, 19, (244, 67, 54, 255))
    px_set(img, 16, 22, (244, 67, 54, 255))
    return img


def gen_flower():
    """Small flower with petals."""
    img = new_img()
    # Stem
    vline(img, 18, 28, 16, (76, 175, 80, 255))
    # Leaves
    px_set(img, 15, 22, (129, 199, 132, 255))
    px_set(img, 17, 24, (129, 199, 132, 255))
    # Petals
    for dx, dy in [(0, -3), (3, -1), (3, 2), (0, 3), (-3, 2), (-3, -1)]:
        px_set(img, 16 + dx, 16 + dy, (255, 235, 59, 255))
    # Center
    px_set(img, 16, 18, (255, 152, 0, 255))
    return img


def gen_dead_bush():
    """Dead brown bush."""
    img = new_img()
    # Sticks
    for x in range(10, 23, 3):
        vline(img, 18, 28, x, (121, 85, 72, 255))
    # Dead branches
    for i in range(5):
        bx = 12 + i * 3
        by = 16 + i
        px_set(img, bx, by, (141, 110, 99, 255))
        px_set(img, bx + 1, by - 1, (141, 110, 99, 255))
    # Base
    rect(img, 10, 26, 12, 3, (93, 64, 55, 255))
    return img


def gen_cactus():
    """Saguaro cactus."""
    img = new_img()
    # Main body
    rect(img, 14, 8, 4, 20, (46, 125, 50, 255))
    # Left arm
    rect(img, 8, 14, 6, 3, (46, 125, 50, 255))
    rect(img, 8, 10, 3, 4, (46, 125, 50, 255))
    # Right arm
    rect(img, 18, 18, 6, 3, (46, 125, 50, 255))
    rect(img, 21, 14, 3, 4, (46, 125, 50, 255))
    # Spines
    px_set(img, 13, 12, (200, 230, 200, 255))
    px_set(img, 18, 10, (200, 230, 200, 255))
    px_set(img, 18, 22, (200, 230, 200, 255))
    # Flowers on top
    px_set(img, 15, 7, (255, 235, 59, 255))
    px_set(img, 16, 7, (255, 235, 59, 255))
    # Ground shadow
    rect(img, 12, 28, 8, 2, (56, 142, 60, 255))
    return img


# ===========================================================================
# RESOURCES (7)
# ===========================================================================

def gen_iron_ore():
    """Iron ore - grey rocky with orange specks."""
    img = new_img()
    rect(img, 4, 8, 24, 20, (117, 117, 117, 255))
    # Ore veins
    for _ in range(6):
        x, y = 6 + (_ * 4) % 20, 10 + (_ * 7) % 16
        px_set(img, x, y, (141, 110, 99, 255))
        px_set(img, x + 1, y, (141, 110, 99, 255))
    # Highlight
    for y in range(10, 20):
        for x in range(6, 14):
            if (x + y) % 5 == 0:
                px_set(img, x, y, (158, 158, 158, 255))
    return img


def gen_coal():
    """Coal - dark black/grey lump."""
    img = new_img()
    circle(img, 16, 18, 12, (33, 33, 33, 255))
    circle(img, 16, 18, 10, (50, 50, 50, 255))
    # Shine
    px_set(img, 12, 12, (80, 80, 80, 255))
    px_set(img, 13, 11, (80, 80, 80, 255))
    px_set(img, 11, 13, (80, 80, 80, 255))
    return img


def gen_copper_ore():
    """Copper ore - brownish with orange/gold."""
    img = new_img()
    rect(img, 5, 6, 22, 22, (121, 85, 72, 255))
    for _ in range(8):
        x, y = 6 + (_ * 5) % 20, 8 + (_ * 3) % 18
        px_set(img, x, y, (255, 152, 0, 255))
        px_set(img, x + 1, y, (255, 152, 0, 255))
    return img


def gen_gold_ore():
    """Gold ore - grey with yellow/gold veins."""
    img = new_img()
    rect(img, 3, 8, 26, 20, (97, 97, 97, 255))
    for _ in range(5):
        gy = 10 + _ * 4
        for gx in range(5, 27):
            px_set(img, gx, gy, (255, 215, 0, 255))
    # Nuggets
    for _ in range(4):
        x, y = 6 + _ * 7, 12 + _ * 3
        px_set(img, x, y, (255, 235, 59, 255))
    return img


def gen_clay():
    """Clay deposit - reddish-brown smooth."""
    img = new_img()
    # Mound shape
    for y in range(8, 30):
        for x in range(4, 28):
            dx, dy = x - 16, y - 20
            if dx * dx + dy * dy * 1.5 < 120:
                n = (x * 3 + y * 7) % 10
                if n < 3:
                    px_set(img, x, y, (160, 100, 70, 255))
                elif n < 7:
                    px_set(img, x, y, (180, 120, 90, 255))
                else:
                    px_set(img, x, y, (140, 80, 50, 255))
    # Cracks
    for _ in range(3):
        cx, cy = 8 + _ * 8, 12 + (_ * 5) % 12
        px_set(img, cx, cy, (120, 70, 40, 255))
        px_set(img, cx + 1, cy + 1, (120, 70, 40, 255))
    return img


def gen_sand():
    """Sand deposit - yellow/tan pile."""
    img = new_img()
    circle(img, 16, 20, 13, (255, 224, 178, 255))
    circle(img, 16, 20, 11, (255, 235, 200, 255))
    # Texture dots
    for _ in range(15):
        x, y = 6 + (_ * 7) % 20, 10 + (_ * 11) % 18
        px_set(img, x, y, (255, 241, 210, 255))
    return img


def gen_stone():
    """Stone deposit - grey rock pile."""
    img = new_img()
    # Three rocks
    circle(img, 12, 22, 8, (158, 158, 158, 255))
    circle(img, 22, 20, 6, (189, 189, 189, 255))
    circle(img, 8, 14, 5, (117, 117, 117, 255))
    # Shadows
    rect(img, 4, 26, 24, 4, (66, 66, 66, 60))
    return img


# ===========================================================================
# FEATURES (7)
# ===========================================================================

def gen_rock_formation():
    """Tall rock formation."""
    img = new_img()
    # Main rock tower
    tri(img, 16, 4, 8, 28, 24, 28, (117, 117, 117, 255))
    rect(img, 10, 14, 12, 14, (158, 158, 158, 255))
    # Left rock
    circle(img, 8, 24, 6, (97, 97, 97, 255))
    circle(img, 8, 24, 5, (117, 117, 117, 255))
    # Right rock
    circle(img, 24, 26, 5, (97, 97, 97, 255))
    # Moss
    px_set(img, 10, 24, (76, 175, 80, 255))
    px_set(img, 11, 25, (76, 175, 80, 255))
    px_set(img, 22, 26, (76, 175, 80, 255))
    return img


def gen_ruins():
    """Ancient ruins - stone columns."""
    img = new_img()
    # Base ground
    rect(img, 2, 24, 28, 6, (121, 85, 72, 255))
    # Left column
    rect(img, 4, 8, 5, 16, (158, 158, 158, 255))
    rect(img, 3, 8, 7, 3, (189, 189, 189, 255))
    # Right column (broken)
    rect(img, 22, 12, 5, 12, (158, 158, 158, 255))
    rect(img, 21, 12, 7, 3, (189, 189, 189, 255))
    # Top lintel (broken)
    rect(img, 4, 5, 16, 3, (117, 117, 117, 255))
    rect(img, 4, 4, 14, 2, (189, 189, 189, 255))
    # Crack
    px_set(img, 16, 6, (97, 97, 97, 255))
    px_set(img, 17, 7, (97, 97, 97, 255))
    # Vines
    px_set(img, 6, 14, (76, 175, 80, 255))
    px_set(img, 6, 15, (76, 175, 80, 255))
    px_set(img, 24, 16, (76, 175, 80, 255))
    return img


def gen_ancient_tree():
    """Massive ancient tree."""
    img = new_img()
    # Wide trunk
    rect(img, 10, 16, 12, 14, (93, 64, 55, 255))
    rect(img, 8, 20, 16, 10, (121, 85, 72, 255))
    # Roots
    px_set(img, 6, 26, (93, 64, 55, 255))
    px_set(img, 7, 27, (93, 64, 55, 255))
    px_set(img, 25, 26, (93, 64, 55, 255))
    px_set(img, 24, 27, (93, 64, 55, 255))
    # Massive canopy
    circle(img, 16, 10, 12, (27, 94, 32, 255))
    circle(img, 16, 10, 10, (46, 125, 50, 255))
    circle(img, 14, 8, 8, (76, 160, 80, 255))
    # Highlights
    px_set(img, 10, 5, (129, 199, 132, 255))
    px_set(img, 20, 6, (129, 199, 132, 255))
    # Eyes in trunk (ancient face)
    px_set(img, 13, 20, (255, 235, 59, 255))
    px_set(img, 19, 20, (255, 235, 59, 255))
    return img


def gen_hot_spring():
    """Hot spring pool with steam."""
    img = new_img()
    # Pool
    circle(img, 16, 20, 14, (100, 181, 246, 255))
    circle(img, 16, 20, 12, (129, 212, 250, 255))
    circle(img, 16, 20, 8, (187, 222, 251, 255))
    # Steam wisps
    for _ in range(4):
        sx, sy = 8 + _ * 6, 6 + (_ * 2) % 6
        px_set(img, sx, sy, (255, 255, 255, 180))
        px_set(img, sx + 1, sy - 1, (255, 255, 255, 160))
        px_set(img, sx - 1, sy - 1, (255, 255, 255, 160))
    # Rocks around edge
    px_set(img, 4, 18, (117, 117, 117, 255))
    px_set(img, 6, 26, (158, 158, 158, 255))
    px_set(img, 26, 20, (117, 117, 117, 255))
    px_set(img, 24, 28, (158, 158, 158, 255))
    px_set(img, 14, 30, (97, 97, 97, 255))
    return img


def gen_geyser():
    """Erupting geyser."""
    img = new_img()
    # Ground cone
    tri(img, 16, 20, 6, 28, 26, 28, (117, 117, 117, 255))
    rect(img, 8, 28, 16, 4, (97, 97, 97, 255))
    # Water jet
    for y in range(4, 20):
        spray = (y % 4) - 2
        px_set(img, 16 + spray, y, (187, 222, 251, 255))
        px_set(img, 16 + spray + 1, y, (255, 255, 255, 200))
    # Splash at top
    for dx in range(-4, 5):
        for dy in range(-2, 3):
            if dx * dx + dy * dy < 8:
                px_set(img, 16 + dx, 2 + dy, (187, 222, 251, 255))
    return img


def gen_meteor_crater():
    """Meteor impact crater."""
    img = new_img()
    # Dark crater
    circle(img, 16, 18, 13, (66, 66, 66, 255))
    circle(img, 16, 18, 10, (50, 50, 50, 255))
    circle(img, 16, 18, 7, (33, 33, 33, 255))
    # Rim
    for _ in range(12):
        angle = _ * 30
        import math as m
        rx = 16 + int(m.cos(m.radians(angle)) * 14)
        ry = 18 + int(m.sin(m.radians(angle)) * 14)
        px_set(img, rx, ry, (158, 158, 158, 255))
    # Meteor fragment
    px_set(img, 16, 18, (255, 152, 0, 255))
    px_set(img, 15, 17, (255, 235, 59, 255))
    # Glow
    px_set(img, 16, 16, (255, 255, 255, 200))
    return img


def gen_fossil():
    """Fossil embedded in rock."""
    img = new_img()
    # Rock slab
    rect(img, 2, 10, 28, 18, (158, 158, 158, 255))
    # Fossil skeleton
    # Spine
    vline(img, 12, 22, 16, (207, 190, 170, 255))
    # Ribs
    for ry in [14, 16, 18, 20]:
        px_set(img, 12, ry, (207, 190, 170, 255))
        px_set(img, 13, ry, (207, 190, 170, 255))
        px_set(img, 19, ry, (207, 190, 170, 255))
        px_set(img, 20, ry, (207, 190, 170, 255))
    # Skull
    circle(img, 16, 10, 4, (207, 190, 170, 255))
    px_set(img, 15, 10, (50, 50, 50, 255))
    px_set(img, 17, 10, (50, 50, 50, 255))
    # Tail
    for t in range(3):
        px_set(img, 16 + t, 23 + t, (207, 190, 170, 255))
    return img


# ===========================================================================
# BUILDINGS (5) — Enhanced pixel art
# ===========================================================================

def gen_house():
    """Detailed wooden house with shingle roof and warm windows."""
    img = new_img()
    # --- WALLS ---
    rect(img, 3, 14, 26, 17, (188, 150, 130, 255))
    # Wood grain (horizontal lines)
    for wy in [16, 18, 20, 22, 24, 26, 28]:
        hline(img, 3, 28, wy, (170, 132, 112, 255))
    # Vertical plank lines
    for wx in [7, 13, 19, 25]:
        vline(img, 14, 30, wx, (165, 127, 107, 255))
    # Wall shadow near bottom
    rect(img, 3, 29, 26, 2, (155, 118, 98, 255))

    # --- ROOF ---
    tri(img, 16, 2, 0, 15, 32, 15, (130, 55, 30, 255))
    tri(img, 16, 3, 2, 15, 30, 15, (155, 75, 45, 255))
    # Shingle rows (alternating light/dark)
    for sy in range(5, 14, 2):
        left = 2 + (14 - sy) * 2
        right = 30 - (14 - sy) * 2
        hline(img, left, right, sy, (150, 68, 38, 255))
        hline(img, left + 1, right - 1, sy + 1, (115, 45, 22, 255))
    # Roof edge highlight
    hline(img, 0, 31, 14, (170, 88, 55, 255))

    # --- CHIMNEY ---
    rect(img, 22, 4, 4, 10, (140, 70, 50, 255))
    px_set(img, 22, 5, (160, 88, 62, 255))
    px_set(img, 23, 7, (160, 88, 62, 255))
    px_set(img, 24, 5, (160, 88, 62, 255))
    px_set(img, 22, 9, (160, 88, 62, 255))
    px_set(img, 24, 9, (160, 88, 62, 255))
    px_set(img, 23, 11, (160, 88, 62, 255))
    rect(img, 21, 3, 6, 2, (120, 55, 35, 255))
    rect(img, 22, 3, 4, 1, (160, 88, 62, 255))
    # Smoke
    px_set(img, 23, 2, (180, 180, 180, 180))
    px_set(img, 24, 1, (200, 200, 200, 120))

    # --- DOOR ---
    rect(img, 13, 21, 6, 9, (101, 67, 33, 255))
    hline(img, 13, 18, 24, (85, 52, 22, 255))
    hline(img, 13, 18, 27, (85, 52, 22, 255))
    # Door handle
    px_set(img, 17, 25, (220, 180, 70, 255))
    # Door frame
    rect(img, 12, 21, 1, 9, (80, 48, 20, 255))
    rect(img, 19, 21, 1, 9, (80, 48, 20, 255))
    rect(img, 12, 20, 8, 1, (80, 48, 20, 255))
    rect(img, 12, 30, 8, 1, (130, 88, 55, 255))

    # --- LEFT WINDOW ---
    rect(img, 6, 17, 4, 5, (255, 210, 80, 255))
    rect(img, 6, 17, 4, 5, (255, 235, 150, 180))
    vline(img, 17, 21, 8, (101, 67, 33, 255))
    hline(img, 6, 9, 19, (101, 67, 33, 255))
    rect(img, 5, 16, 6, 1, (101, 67, 33, 255))
    rect(img, 5, 22, 6, 1, (101, 67, 33, 255))
    rect(img, 5, 17, 1, 5, (101, 67, 33, 255))
    rect(img, 10, 17, 1, 5, (101, 67, 33, 255))
    px_set(img, 7, 18, (255, 248, 200, 255))
    px_set(img, 8, 20, (255, 248, 200, 255))

    # --- RIGHT WINDOW ---
    rect(img, 22, 17, 4, 5, (255, 210, 80, 255))
    rect(img, 22, 17, 4, 5, (255, 235, 150, 180))
    vline(img, 17, 21, 24, (101, 67, 33, 255))
    hline(img, 22, 25, 19, (101, 67, 33, 255))
    rect(img, 21, 16, 6, 1, (101, 67, 33, 255))
    rect(img, 21, 22, 6, 1, (101, 67, 33, 255))
    rect(img, 21, 17, 1, 5, (101, 67, 33, 255))
    rect(img, 26, 17, 1, 5, (101, 67, 33, 255))
    px_set(img, 23, 18, (255, 248, 200, 255))
    px_set(img, 24, 20, (255, 248, 200, 255))

    # --- FOUNDATION ---
    rect(img, 2, 30, 28, 2, (125, 82, 55, 255))
    hline(img, 2, 29, 30, (145, 98, 68, 255))
    return img


def gen_stone_house():
    """Detailed stone house with block texture and slate roof."""
    img = new_img()
    # --- WALLS ---
    rect(img, 3, 14, 26, 17, (158, 158, 158, 255))
    # Stone blocks (individual rectangles in staggered rows)
    # Row 1 (y=15)
    for sx in [3, 8, 13, 18, 23]:
        rect(img, sx, 15, 4, 3, (130, 130, 130, 255))
        px_set(img, sx, 15, (172, 172, 172, 255))
    # Row 2 (y=18) offset
    for sx in [5, 10, 15, 20]:
        rect(img, sx, 18, 4, 3, (130, 130, 130, 255))
        px_set(img, sx, 18, (172, 172, 172, 255))
    # Row 3 (y=21)
    for sx in [3, 8, 13, 18, 23]:
        rect(img, sx, 21, 4, 3, (130, 130, 130, 255))
        px_set(img, sx, 21, (172, 172, 172, 255))
    # Row 4 (y=24) offset
    for sx in [5, 10, 15, 20, 25]:
        rect(img, sx, 24, 4, 3, (130, 130, 130, 255))
        px_set(img, sx, 24, (172, 172, 172, 255))
    # Row 5 (y=27)
    for sx in [3, 8, 13, 18, 23]:
        rect(img, sx, 27, 4, 3, (130, 130, 130, 255))
        px_set(img, sx, 27, (172, 172, 172, 255))
    # Mortar lines (vertical gaps between blocks)
    for vx in [7, 12, 17, 22]:
        vline(img, 15, 29, vx, (97, 97, 97, 255))
    for vx in [9, 14, 19]:
        vline(img, 18, 20, vx, (97, 97, 97, 255))
        vline(img, 24, 26, vx, (97, 97, 97, 255))

    # --- ROOF ---
    tri(img, 16, 2, 0, 15, 32, 15, (50, 50, 50, 255))
    tri(img, 16, 3, 2, 15, 30, 15, (66, 66, 66, 255))
    # Slate tile rows
    for sy in range(5, 14, 2):
        left = 2 + (14 - sy) * 2
        right = 30 - (14 - sy) * 2
        hline(img, left, right, sy, (58, 58, 58, 255))
        hline(img, left + 1, right - 1, sy + 1, (42, 42, 42, 255))
    # Roof edge
    hline(img, 0, 31, 14, (75, 75, 75, 255))

    # --- DOOR (arched) ---
    rect(img, 13, 22, 6, 8, (80, 50, 30, 255))
    px_set(img, 14, 21, (80, 50, 30, 255))
    px_set(img, 17, 21, (80, 50, 30, 255))
    # Door panels
    hline(img, 14, 17, 25, (65, 38, 20, 255))
    hline(img, 14, 17, 28, (65, 38, 20, 255))
    # Door handle
    px_set(img, 17, 26, (200, 170, 60, 255))
    # Stone arch frame
    rect(img, 12, 22, 1, 8, (117, 117, 117, 255))
    rect(img, 19, 22, 1, 8, (117, 117, 117, 255))
    rect(img, 12, 21, 8, 1, (117, 117, 117, 255))

    # --- WINDOWS ---
    for wx in [6, 22]:
        rect(img, wx, 17, 4, 4, (255, 210, 80, 255))
        vline(img, 17, 20, wx + 2, (101, 67, 33, 255))
        hline(img, wx, wx + 3, 19, (101, 67, 33, 255))
        # Stone window frame
        rect(img, wx - 1, 16, 6, 1, (117, 117, 117, 255))
        rect(img, wx - 1, 21, 6, 1, (117, 117, 117, 255))
        rect(img, wx - 1, 17, 1, 4, (117, 117, 117, 255))
        rect(img, wx + 4, 17, 1, 4, (117, 117, 117, 255))

    # --- MOSS details ---
    px_set(img, 4, 28, (76, 145, 65, 255))
    px_set(img, 4, 29, (56, 125, 45, 255))
    px_set(img, 3, 28, (76, 145, 65, 255))
    px_set(img, 27, 27, (76, 145, 65, 255))

    # --- FOUNDATION ---
    rect(img, 2, 30, 28, 2, (97, 97, 97, 255))
    hline(img, 2, 29, 30, (117, 117, 117, 255))
    return img


def gen_watchtower():
    """Detailed wooden watchtower with platform and ladder."""
    img = new_img()
    # --- TOWER LEGS ---
    rect(img, 5, 18, 4, 13, (101, 67, 33, 255))
    rect(img, 23, 18, 4, 13, (101, 67, 33, 255))
    # Leg highlights
    rect(img, 5, 18, 1, 13, (130, 88, 55, 255))
    rect(img, 23, 18, 1, 13, (130, 88, 55, 255))
    # Cross bracing
    px_set(img, 9, 20, (101, 67, 33, 255))
    px_set(img, 10, 21, (101, 67, 33, 255))
    px_set(img, 11, 22, (101, 67, 33, 255))
    px_set(img, 12, 23, (101, 67, 33, 255))
    px_set(img, 22, 20, (101, 67, 33, 255))
    px_set(img, 21, 21, (101, 67, 33, 255))
    px_set(img, 20, 22, (101, 67, 33, 255))
    px_set(img, 19, 23, (101, 67, 33, 255))

    # --- LADDER ---
    vline(img, 18, 29, 14, (101, 67, 33, 255))
    vline(img, 18, 29, 18, (101, 67, 33, 255))
    for ly in [19, 21, 23, 25, 27]:
        hline(img, 14, 18, ly, (101, 67, 33, 255))
    # Ladder highlight
    vline(img, 18, 29, 14, (130, 88, 55, 255))

    # --- PLATFORM ---
    rect(img, 3, 16, 26, 3, (150, 75, 50, 255))
    hline(img, 3, 28, 16, (175, 95, 65, 255))
    # Platform support beams
    rect(img, 3, 18, 2, 1, (120, 60, 40, 255))
    rect(img, 27, 18, 2, 1, (120, 60, 40, 255))
    # Railing
    rect(img, 3, 13, 26, 1, (101, 67, 33, 255))
    # Railing posts
    for rx in [4, 10, 16, 22, 28]:
        vline(img, 13, 16, rx, (101, 67, 33, 255))

    # --- UPPER CABIN ---
    rect(img, 7, 8, 18, 6, (188, 150, 130, 255))
    # Cabin wood lines
    for wy in [10, 12]:
        hline(img, 7, 24, wy, (170, 132, 112, 255))
    # Cabin shadow
    rect(img, 7, 13, 18, 1, (160, 125, 105, 255))

    # --- CABIN ROOF ---
    tri(img, 16, 2, 4, 8, 28, 8, (150, 75, 50, 255))
    tri(img, 16, 3, 6, 8, 26, 8, (175, 92, 60, 255))
    # Flag pole
    vline(img, 0, 3, 16, (101, 67, 33, 255))
    px_set(img, 15, 1, (200, 50, 40, 255))
    px_set(img, 16, 1, (200, 50, 40, 255))
    px_set(img, 15, 2, (200, 50, 40, 255))

    # --- LOOKOUT WINDOW ---
    rect(img, 14, 9, 4, 3, (255, 210, 80, 255))
    vline(img, 9, 11, 16, (101, 67, 33, 255))
    hline(img, 14, 17, 10, (101, 67, 33, 255))
    # Window frame
    rect(img, 13, 8, 6, 1, (101, 67, 33, 255))

    # --- GROUND SHADOW ---
    hline(img, 3, 27, 30, (80, 55, 30, 80))
    hline(img, 4, 26, 31, (80, 55, 30, 50))
    return img


def gen_workshop():
    """Detailed blacksmith workshop with forge glow."""
    img = new_img()
    # --- WALLS ---
    rect(img, 2, 14, 28, 17, (141, 110, 99, 255))
    # Horizontal wood lines
    for wy in [16, 18, 20, 22, 24, 26, 28]:
        hline(img, 2, 29, wy, (125, 95, 84, 255))

    # --- ROOF ---
    rect(img, 1, 11, 30, 4, (117, 60, 40, 255))
    rect(img, 0, 13, 32, 2, (145, 75, 50, 255))
    # Roof tiles
    for tx in range(2, 30, 4):
        rect(img, tx, 11, 3, 2, (130, 68, 45, 255))
    # Roof edge
    hline(img, 0, 31, 14, (155, 82, 55, 255))

    # --- CHIMNEY ---
    rect(img, 25, 4, 4, 8, (140, 70, 50, 255))
    # Brick pattern
    px_set(img, 25, 5, (160, 88, 62, 255))
    px_set(img, 27, 5, (160, 88, 62, 255))
    px_set(img, 26, 7, (160, 88, 62, 255))
    px_set(img, 25, 9, (160, 88, 62, 255))
    px_set(img, 27, 9, (160, 88, 62, 255))
    # Chimney cap
    rect(img, 24, 3, 6, 2, (117, 55, 35, 255))
    # Smoke
    px_set(img, 26, 2, (140, 140, 140, 180))
    px_set(img, 27, 1, (160, 160, 160, 140))
    px_set(img, 25, 1, (160, 160, 160, 120))

    # --- LARGE OPEN DOUBLE DOOR ---
    rect(img, 9, 18, 14, 13, (30, 20, 10, 255))
    # Interior forge glow (red/orange)
    rect(img, 12, 20, 8, 8, (180, 60, 30, 200))
    rect(img, 14, 22, 4, 4, (230, 120, 40, 255))
    rect(img, 15, 23, 2, 2, (255, 180, 60, 255))
    # Door frame
    rect(img, 8, 18, 1, 13, (101, 67, 33, 255))
    rect(img, 23, 18, 1, 13, (101, 67, 33, 255))
    rect(img, 8, 17, 16, 1, (101, 67, 33, 255))
    # Door on sides (partially open)
    rect(img, 9, 18, 2, 13, (101, 67, 33, 255))
    rect(img, 21, 18, 2, 13, (101, 67, 33, 255))
    # Door hinges
    px_set(img, 9, 20, (60, 60, 60, 255))
    px_set(img, 9, 27, (60, 60, 60, 255))
    px_set(img, 22, 20, (60, 60, 60, 255))
    px_set(img, 22, 27, (60, 60, 60, 255))

    # --- ANVIL ---
    rect(img, 15, 26, 3, 2, (80, 80, 80, 255))
    px_set(img, 16, 25, (80, 80, 80, 255))
    rect(img, 14, 28, 5, 1, (60, 60, 60, 255))

    # --- WINDOW (side) ---
    rect(img, 4, 16, 3, 3, (255, 210, 80, 255))
    rect(img, 4, 16, 3, 3, (255, 235, 150, 150))
    hline(img, 4, 6, 18, (101, 67, 33, 255))
    vline(img, 16, 18, 5, (101, 67, 33, 255))

    # --- FOUNDATION ---
    rect(img, 1, 30, 30, 2, (117, 75, 55, 255))
    hline(img, 1, 30, 30, (140, 92, 68, 255))
    return img


def gen_well():
    """Detailed stone well with roof and bucket."""
    img = new_img()
    # --- WELL BASE (circular stone) ---
    # Outer ring
    rect(img, 8, 18, 16, 11, (158, 158, 158, 255))
    # Stone blocks on well body
    for sx in [8, 12, 16, 20]:
        rect(img, sx, 18, 3, 3, (130, 130, 130, 255))
    for sx in [10, 14, 18, 22]:
        rect(img, sx, 21, 3, 3, (130, 130, 130, 255))
    for sx in [8, 12, 16, 20]:
        rect(img, sx, 24, 3, 3, (130, 130, 130, 255))
    for sx in [10, 14, 18, 22]:
        rect(img, sx, 27, 3, 3, (130, 130, 130, 255))
    # Top rim
    rect(img, 8, 17, 16, 2, (189, 189, 189, 255))
    hline(img, 8, 23, 17, (210, 210, 210, 255))
    # Bottom rim
    rect(img, 8, 28, 16, 2, (189, 189, 189, 255))

    # --- INTERIOR (dark water) ---
    rect(img, 12, 19, 8, 8, (15, 15, 30, 255))
    # Water surface
    hline(img, 12, 19, 26, (30, 60, 100, 255))
    px_set(img, 14, 26, (50, 100, 150, 255))
    px_set(img, 17, 26, (50, 100, 150, 255))

    # --- CORNER POSTS ---
    vline(img, 14, 19, 10, (101, 67, 33, 255))
    vline(img, 14, 19, 22, (101, 67, 33, 255))
    # Post highlights
    px_set(img, 10, 14, (130, 88, 55, 255))
    px_set(img, 22, 14, (130, 88, 55, 255))

    # --- CROSSBEAM ---
    hline(img, 10, 22, 14, (101, 67, 33, 255))
    hline(img, 10, 22, 15, (80, 50, 25, 255))

    # --- ROOF ---
    tri(img, 16, 6, 6, 16, 26, 16, (150, 75, 50, 255))
    tri(img, 16, 7, 8, 16, 24, 16, (175, 92, 60, 255))
    # Roof tiles
    for ry in [8, 10, 12, 14]:
        left = 6 + (16 - ry) * 2
        right = 26 - (16 - ry) * 2
        hline(img, left, right, ry, (140, 68, 42, 255))
    # Roof peak
    px_set(img, 16, 6, (160, 82, 55, 255))

    # --- BUCKET AND ROPE ---
    # Chain/rope
    px_set(img, 16, 16, (141, 110, 99, 255))
    px_set(img, 16, 17, (141, 110, 99, 255))
    px_set(img, 16, 18, (141, 110, 99, 255))
    # Bucket
    rect(img, 15, 19, 3, 3, (101, 67, 33, 255))
    rect(img, 14, 19, 5, 1, (120, 80, 48, 255))
    px_set(img, 14, 20, (101, 67, 33, 255))
    px_set(img, 18, 20, (101, 67, 33, 255))
    px_set(img, 14, 21, (101, 67, 33, 255))
    px_set(img, 18, 21, (101, 67, 33, 255))

    # --- GROUND BASE ---
    rect(img, 6, 29, 20, 3, (141, 110, 99, 255))
    hline(img, 6, 25, 29, (160, 125, 110, 255))
    # Grass tufts around base
    px_set(img, 7, 30, (76, 160, 70, 255))
    px_set(img, 24, 30, (76, 160, 70, 255))
    px_set(img, 25, 31, (56, 140, 50, 255))
    return img


# ===========================================================================
# CHARACTERS (4)
# ===========================================================================

def gen_character_male():
    """Male character - blue clothes."""
    img = new_img()
    # Head
    circle(img, 16, 6, 5, (255, 224, 178, 255))
    # Hair
    circle(img, 16, 4, 5, (50, 30, 15, 255))
    # Eyes
    px_set(img, 14, 6, (33, 33, 33, 255))
    px_set(img, 18, 6, (33, 33, 33, 255))
    # Body
    rect(img, 12, 12, 8, 8, (33, 150, 243, 255))
    # Arms
    rect(img, 8, 12, 4, 6, (255, 224, 178, 255))
    rect(img, 20, 12, 4, 6, (255, 224, 178, 255))
    # Legs
    rect(img, 12, 20, 3, 8, (33, 33, 33, 255))
    rect(img, 17, 20, 3, 8, (33, 33, 33, 255))
    # Shoes
    rect(img, 11, 27, 4, 2, (97, 97, 97, 255))
    rect(img, 17, 27, 4, 2, (97, 97, 97, 255))
    return img


def gen_character_female():
    """Female character - red/pink clothes."""
    img = new_img()
    # Head
    circle(img, 16, 6, 5, (255, 224, 178, 255))
    # Hair (longer)
    circle(img, 16, 4, 5, (100, 50, 20, 255))
    px_set(img, 13, 7, (100, 50, 20, 255))
    px_set(img, 19, 7, (100, 50, 20, 255))
    px_set(img, 13, 8, (100, 50, 20, 255))
    px_set(img, 19, 8, (100, 50, 20, 255))
    # Eyes
    px_set(img, 14, 6, (33, 33, 33, 255))
    px_set(img, 18, 6, (33, 33, 33, 255))
    # Body (dress)
    rect(img, 11, 12, 10, 8, (233, 30, 99, 255))
    tri(img, 11, 20, 21, 20, 16, 28, (233, 30, 99, 255))
    # Arms
    rect(img, 8, 12, 3, 6, (255, 224, 178, 255))
    rect(img, 21, 12, 3, 6, (255, 224, 178, 255))
    # Shoes
    rect(img, 12, 27, 4, 2, (255, 255, 255, 255))
    rect(img, 17, 27, 4, 2, (255, 255, 255, 255))
    return img


def gen_character_child():
    """Child character - small."""
    img = new_img()
    # Head (proportionally larger)
    circle(img, 16, 8, 6, (255, 224, 178, 255))
    # Hair
    circle(img, 16, 6, 6, (200, 150, 50, 255))
    # Eyes
    px_set(img, 14, 8, (33, 33, 33, 255))
    px_set(img, 18, 8, (33, 33, 33, 255))
    # Small body
    rect(img, 13, 14, 6, 6, (255, 235, 59, 255))
    # Legs
    rect(img, 13, 20, 2, 6, (33, 150, 243, 255))
    rect(img, 17, 20, 2, 6, (33, 150, 243, 255))
    # Shoes
    rect(img, 12, 26, 3, 2, (255, 255, 255, 255))
    rect(img, 17, 26, 3, 2, (255, 255, 255, 255))
    return img


def gen_character_guard():
    """Guard character - armor."""
    img = new_img()
    # Head (helmet)
    circle(img, 16, 6, 5, (200, 180, 140, 255))
    rect(img, 11, 3, 10, 4, (158, 158, 158, 255))
    # Visor
    hline(img, 12, 20, 6, (33, 33, 33, 255))
    # Body (armor)
    rect(img, 11, 12, 10, 8, (158, 158, 158, 255))
    rect(img, 12, 12, 8, 3, (189, 189, 189, 255))
    # Arms
    rect(img, 8, 12, 3, 6, (158, 158, 158, 255))
    rect(img, 21, 12, 3, 6, (158, 158, 158, 255))
    # Legs
    rect(img, 12, 20, 3, 8, (117, 117, 117, 255))
    rect(img, 17, 20, 3, 8, (117, 117, 117, 255))
    # Boots
    rect(img, 11, 27, 5, 2, (50, 50, 50, 255))
    rect(img, 17, 27, 5, 2, (50, 50, 50, 255))
    # Spear
    vline(img, 6, 20, 4, (101, 67, 33, 255))
    px_set(img, 4, 4, (158, 158, 158, 255))
    px_set(img, 4, 5, (158, 158, 158, 255))
    return img


# ===========================================================================
# MISC (7)
# ===========================================================================

def gen_shop():
    """Detailed market shop with awning and sign."""
    img = new_img()
    # --- WALLS ---
    rect(img, 3, 14, 26, 17, (188, 150, 130, 255))
    # Wood plank lines
    for wy in [16, 18, 20, 22, 24, 26, 28]:
        hline(img, 3, 28, wy, (170, 132, 112, 255))
    # Vertical plank seams
    for wx in [8, 15, 22]:
        vline(img, 18, 30, wx, (165, 127, 107, 255))
    # Wall shadow
    rect(img, 3, 29, 26, 2, (155, 118, 98, 255))

    # --- ROOF ---
    tri(img, 16, 2, 0, 14, 32, 14, (150, 75, 50, 255))
    tri(img, 16, 3, 2, 14, 30, 14, (175, 92, 60, 255))
    # Roof tiles
    for sy in range(5, 13, 2):
        left = 2 + (14 - sy) * 2
        right = 30 - (14 - sy) * 2
        hline(img, left, right, sy, (140, 68, 42, 255))
        hline(img, left + 1, right - 1, sy + 1, (120, 55, 32, 255))

    # --- AWNING (red/white stripes) ---
    for ax in range(2, 28, 4):
        rect(img, ax, 14, 2, 4, (244, 67, 54, 255))
        rect(img, ax + 2, 14, 2, 4, (255, 255, 255, 255))
    # Awning shadow
    hline(img, 2, 29, 17, (180, 40, 30, 255))
    # Awning scalloped edge
    for sx in range(3, 28, 4):
        px_set(img, sx, 18, (244, 67, 54, 255))
        px_set(img, sx + 2, 18, (255, 255, 255, 255))

    # --- SIGN ---
    rect(img, 12, 9, 8, 4, (255, 255, 255, 255))
    rect(img, 12, 10, 8, 3, (200, 180, 160, 255))
    # Sign text (dots)
    px_set(img, 14, 11, (244, 67, 54, 255))
    px_set(img, 16, 11, (244, 67, 54, 255))
    px_set(img, 18, 11, (244, 67, 54, 255))
    # Sign chains
    px_set(img, 13, 8, (180, 150, 80, 255))
    px_set(img, 19, 8, (180, 150, 80, 255))
    # Sign border
    rect(img, 11, 8, 10, 1, (101, 67, 33, 255))
    rect(img, 11, 13, 10, 1, (101, 67, 33, 255))
    rect(img, 11, 9, 1, 4, (101, 67, 33, 255))
    rect(img, 20, 9, 1, 4, (101, 67, 33, 255))

    # --- DISPLAY WINDOW ---
    rect(img, 5, 18, 5, 6, (100, 180, 230, 255))
    # Window frame (cross)
    vline(img, 18, 23, 7, (101, 67, 33, 255))
    vline(img, 18, 23, 8, (101, 67, 33, 255))
    hline(img, 5, 9, 21, (101, 67, 33, 255))
    # Window border
    rect(img, 4, 17, 7, 1, (101, 67, 33, 255))
    rect(img, 4, 24, 7, 1, (101, 67, 33, 255))
    rect(img, 4, 18, 1, 6, (101, 67, 33, 255))
    rect(img, 10, 18, 1, 6, (101, 67, 33, 255))
    # Goods in window (colorful dots)
    px_set(img, 6, 19, (244, 67, 54, 255))
    px_set(img, 8, 19, (76, 175, 80, 255))
    px_set(img, 6, 22, (255, 193, 7, 255))
    px_set(img, 8, 22, (33, 150, 243, 255))

    # --- OPEN DOOR ---
    rect(img, 14, 20, 6, 10, (50, 35, 20, 255))
    # Interior visible
    rect(img, 15, 21, 4, 7, (160, 130, 100, 255))
    # Door frame
    rect(img, 13, 20, 1, 10, (101, 67, 33, 255))
    rect(img, 20, 20, 1, 10, (101, 67, 33, 255))
    rect(img, 13, 19, 8, 1, (101, 67, 33, 255))
    # Door step
    rect(img, 13, 30, 7, 1, (120, 80, 50, 255))

    # --- FOUNDATION ---
    rect(img, 2, 30, 28, 2, (125, 82, 55, 255))
    hline(img, 2, 29, 30, (145, 98, 68, 255))
    return img


def gen_tombstone():
    """Grave marker."""
    img = new_img()
    # Ground mound
    rect(img, 6, 24, 20, 6, (121, 85, 72, 255))
    # Tombstone
    rect(img, 11, 12, 10, 12, (189, 189, 189, 255))
    # Rounded top
    circle(img, 16, 12, 5, (189, 189, 189, 255))
    # Cross
    vline(img, 16, 22, 14, (117, 117, 117, 255))
    hline(img, 14, 18, 17, (117, 117, 117, 255))
    # Epitaph line
    hline(img, 13, 19, 20, (117, 117, 117, 255))
    # Shadow
    rect(img, 14, 26, 10, 2, (93, 64, 55, 100))
    return img


def gen_road():
    """Road/path tile."""
    img = new_img()
    # Base dirt
    rect(img, 0, 8, 32, 16, (141, 110, 99, 255))
    # Road surface
    rect(img, 0, 10, 32, 12, (158, 130, 110, 255))
    # Cobblestone pattern
    for _ in range(8):
        cx, cy = (_ * 9) % 28, 11 + (_ % 3) * 4
        rect(img, cx, cy, 5, 3, (169, 140, 120, 255))
        rect(img, cx + 1, cy + 1, 3, 1, (141, 110, 99, 255))
    # Edge lines
    hline(img, 0, 31, 10, (121, 85, 72, 255))
    hline(img, 0, 31, 21, (121, 85, 72, 255))
    return img


def gen_farm_fallow():
    """Fallow farmland - bare brown soil."""
    img = new_img()
    rect(img, 0, 0, 32, 32, (141, 110, 99, 255))
    # Plow lines
    for y in range(0, 32, 4):
        hline(img, 0, 31, y, (121, 85, 72, 255))
    # Texture
    for _ in range(20):
        tx, ty = (_ * 7) % 30, (_ * 13) % 30
        px_set(img, tx, ty, (160, 125, 110, 255))
        px_set(img, tx + 1, ty, (130, 95, 85, 255))
    return img


def gen_farm_growing():
    """Growing crops - green sprouts."""
    img = new_img()
    rect(img, 0, 0, 32, 32, (141, 110, 99, 255))
    # Row pattern
    for row in range(4):
        ry = 4 + row * 7
        hline(img, 0, 31, ry, (93, 64, 55, 255))
        # Sprouts in row
        for sx in range(2, 32, 6):
            rect(img, sx, ry - 3, 2, 4, (76, 175, 80, 255))
            px_set(img, sx, ry - 4, (129, 199, 132, 255))
    return img


def gen_farm_weedy():
    """Weedy farmland - overgrown with weeds."""
    img = new_img()
    rect(img, 0, 0, 32, 32, (141, 110, 99, 255))
    # Weeds everywhere
    for _ in range(20):
        wx, wy = (_ * 11) % 30, (_ * 7) % 30
        # Weed clump
        px_set(img, wx, wy, (76, 175, 80, 255))
        px_set(img, wx + 1, wy, (56, 142, 60, 255))
        px_set(img, wx - 1, wy + 1, (46, 125, 50, 255))
        px_set(img, wx, wy - 1, (129, 199, 132, 255))
    # Visible dirt patches
    for _ in range(8):
        dx, dy = (_ * 15) % 28, (_ * 9) % 28
        rect(img, dx, dy, 3, 2, (93, 64, 55, 255))
    return img


def gen_farm_ready():
    """Ready to harvest - golden crops."""
    img = new_img()
    rect(img, 0, 0, 32, 32, (141, 110, 99, 255))
    # Ripe crops
    for row in range(5):
        ry = 3 + row * 6
        hline(img, 0, 31, ry, (93, 64, 55, 255))
        for sx in range(2, 32, 5):
            # Stalk
            rect(img, sx, ry - 4, 1, 6, (255, 193, 7, 255))
            # Grain head
            px_set(img, sx, ry - 5, (255, 235, 59, 255))
            px_set(img, sx - 1, ry - 4, (255, 235, 59, 255))
            px_set(img, sx + 1, ry - 4, (255, 235, 59, 255))
    return img


# ===========================================================================
# MAIN
# ===========================================================================

GENERATORS = {
    # Vegetation
    "vegetation/deciduous_tree.png": gen_deciduous_tree,
    "vegetation/pine_tree.png": gen_pine_tree,
    "vegetation/palm_tree.png": gen_palm_tree,
    "vegetation/bush.png": gen_bush,
    "vegetation/flower.png": gen_flower,
    "vegetation/dead_bush.png": gen_dead_bush,
    "vegetation/cactus.png": gen_cactus,
    # Resources
    "resources/iron_ore.png": gen_iron_ore,
    "resources/coal.png": gen_coal,
    "resources/copper_ore.png": gen_copper_ore,
    "resources/gold_ore.png": gen_gold_ore,
    "resources/clay_deposit.png": gen_clay,
    "resources/sand_deposit.png": gen_sand,
    "resources/stone_deposit.png": gen_stone,
    # Features
    "features/rock_formation.png": gen_rock_formation,
    "features/ancient_ruins.png": gen_ruins,
    "features/ancient_tree.png": gen_ancient_tree,
    "features/hot_spring.png": gen_hot_spring,
    "features/geyser.png": gen_geyser,
    "features/meteor_crater.png": gen_meteor_crater,
    "features/fossil.png": gen_fossil,
    # Buildings
    "buildings/house.png": gen_house,
    "buildings/stone_house.png": gen_stone_house,
    "buildings/watchtower.png": gen_watchtower,
    "buildings/workshop.png": gen_workshop,
    "buildings/well.png": gen_well,
    # Characters
    "characters/character_male.png": gen_character_male,
    "characters/character_female.png": gen_character_female,
    "characters/character_child.png": gen_character_child,
    "characters/character_guard.png": gen_character_guard,
    # Misc
    "misc/shop.png": gen_shop,
    "misc/tombstone.png": gen_tombstone,
    "misc/road_path.png": gen_road,
    "misc/farm_fallow.png": gen_farm_fallow,
    "misc/farm_growing.png": gen_farm_growing,
    "misc/farm_weedy.png": gen_farm_weedy,
    "misc/farm_ready.png": gen_farm_ready,
}


def main():
    print("=" * 50)
    print("Regenerating ALL sprites with clean 32x32 pixel art...")
    print("=" * 50)

    count = 0
    for rel_path, gen_func in GENERATORS.items():
        full_path = os.path.join(OUTPUT_DIR, rel_path)
        os.makedirs(os.path.dirname(full_path), exist_ok=True)
        img = gen_func()
        img.save(full_path)
        count += 1
        print(f"  Generated: {rel_path}")

    # Also save 4x previews
    print("\nSaving previews...")
    for rel_path, gen_func in GENERATORS.items():
        full_path = os.path.join(OUTPUT_DIR, rel_path)
        img = Image.open(full_path)
        preview = img.resize((128, 128), Image.NEAREST)
        preview_path = full_path.replace(".png", "_preview.png")
        preview.save(preview_path)

    print(f"\n{'=' * 50}")
    print(f"Done! Generated {count} sprites as clean 32x32 pixel art")
    print(f"{'=' * 50}")


if __name__ == "__main__":
    main()
