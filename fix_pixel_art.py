"""
Fix transparency on object sprites and regenerate terrain with procedural pixel art.
"""
import os
from PIL import Image
import random
import math

OUTPUT_DIR = "D:/work/rust_len/sim_world/assets/pixel_prototypes"

# ===========================================================================
# STEP 1: Fix transparency on all non-terrain sprites
# ===========================================================================

def fix_transparency(img, tolerance=60):
    """Remove white/light background from a sprite image."""
    w, h = img.size
    pixels = img.load()
    result = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    rpixels = result.load()

    # Determine background color from corners
    corners = [
        pixels[0, 0],
        pixels[w-1, 0],
        pixels[0, h-1],
        pixels[w-1, h-1],
    ]
    # Use median corner color as bg
    rs = sorted([c[0] for c in corners])
    gs = sorted([c[1] for c in corners])
    bs = sorted([c[2] for c in corners])
    bg = (rs[1], gs[1], bs[1])

    for y in range(h):
        for x in range(w):
            px = pixels[x, y]
            if len(px) >= 3:
                dist = ((px[0]-bg[0])**2 + (px[1]-bg[1])**2 + (px[2]-bg[2])**2) ** 0.5
                alpha = 0 if dist < tolerance else 255
            else:
                alpha = 255
            if len(px) == 4:
                rpixels[x, y] = (px[0], px[1], px[2], alpha)
            else:
                rpixels[x, y] = (px[0], px[1], px[2], alpha)
    return result


def fix_object_transparency():
    """Fix transparency on all 32x32 object PNGs (non-terrain)."""
    fix_categories = [
        "vegetation", "resources", "features",
        "buildings", "characters", "misc"
    ]

    count = 0
    for cat in fix_categories:
        cat_dir = os.path.join(OUTPUT_DIR, cat)
        if not os.path.isdir(cat_dir):
            continue
        for fname in os.listdir(cat_dir):
            if not fname.endswith(".png") or "_full" in fname or "_preview" in fname:
                continue
            path = os.path.join(cat_dir, fname)
            try:
                img = Image.open(path).convert("RGBA")
                # Check if already has transparency
                has_alpha = any(p[3] < 255 for p in list(img.getdata())[:20])
                if has_alpha:
                    continue  # skip if already transparent
                fixed = fix_transparency(img, tolerance=55)
                fixed.save(path)
                count += 1
                print(f"  Fixed: {cat}/{fname}")
            except Exception as e:
                print(f"  Error {cat}/{fname}: {e}")

    print(f"Fixed transparency on {count} sprites\n")
    return count


# ===========================================================================
# STEP 2: Seamless pixel art terrain generation
# ===========================================================================
# Uses sine-wave noise (naturally seamless at 32px boundaries with integer
# frequencies) + value noise on a wrapped grid.  Each terrain type uses a
# limited palette (3-6 colors) with hard thresholding — NO color blending,
# so edges stay crisp and tiles don't look muddy.

def sseamless_noise(x, y, seed, freq=4):
    """Multi-octave sine-based noise, naturally seamless at 32px boundaries.
    Uses integer frequencies so sin wraps exactly at x/y = 0 and 32."""
    pi2 = 2.0 * math.pi
    v = 0.0
    v += math.sin((x / 32.0) * pi2 * freq + seed * 0.1)
    v += math.sin((y / 32.0) * pi2 * freq + seed * 0.3)
    v += math.sin(((x + y) / 32.0) * pi2 * (freq + 1) + seed * 0.7)
    v += math.sin(((x - y) / 32.0) * pi2 * max(1, freq - 1) + seed * 1.1)
    return v / 4.0  # range ≈ [-1, 1]


def _wrapped_hash(x, y, seed, grid=8):
    """Deterministic hash with wrapping at 32px boundary.
    Only used for value-noise detail layer."""
    wrap = 32 // grid
    gx = (x // grid) % wrap
    gy = (y // grid) % wrap
    h = (gx * 374761393 + gy * 668265263 + seed * 1274126177) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((h ^ (h >> 16)) & 0xFFFF) / 65536.0


def terrain_noise(x, y, seed):
    """Combined seamless noise for terrain. Returns value in [0, 1]."""
    # 4-octave sine blend (naturally seamless)
    n1 = sseamless_noise(x, y, seed, freq=2) * 0.5
    n2 = sseamless_noise(x, y, seed + 100, freq=4) * 0.3
    n3 = sseamless_noise(x, y, seed + 200, freq=8) * 0.15
    # Tiny bit of wrapped value noise for detail
    n4 = (_wrapped_hash(x, y, seed + 300, grid=4) - 0.5) * 0.05
    val = n1 + n2 + n3 + n4
    return max(0.0, min(1.0, val * 0.5 + 0.5))


def pick(val, palette):
    """Pick a colour from palette based on 0-1 value (hard threshold)."""
    idx = min(int(val * len(palette)), len(palette) - 1)
    return palette[idx]


def make_terrain(name, palette, freq_mod=0):
    """Factory that builds a terrain generator with the given palette."""
    def gen(sr_unused, seed):
        img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
        px = img.load()
        s = seed + freq_mod
        for y in range(32):
            for x in range(32):
                val = terrain_noise(x, y, s)
                px[x, y] = pick(val, palette)
        return img
    gen.__name__ = f"gen_terrain_{name}"
    return gen


# Palettes — each is 3-6 distinct colours with NO blending.
GRASS    = [(102, 187, 106, 255), (76, 175, 80, 255), (56, 142, 60, 255)]
WATER    = [(25, 118, 210, 255), (33, 150, 243, 255), (100, 181, 246, 255)]
DEEP_H2O = [(12, 36, 97, 255), (13, 71, 161, 255), (21, 101, 192, 255)]
SAND     = [(255, 241, 210, 255), (255, 224, 178, 255), (209, 170, 120, 255)]
FOREST   = [(13, 60, 20, 255), (27, 94, 32, 255), (46, 125, 50, 255), (76, 160, 80, 255)]
SWAMP    = [(40, 55, 20, 255), (69, 90, 40, 255), (104, 125, 55, 255), (55, 75, 50, 255)]
STONE    = [(66, 66, 66, 255), (97, 97, 97, 255), (117, 117, 117, 255), (158, 158, 158, 255)]
DIRT     = [(93, 64, 55, 255), (121, 85, 72, 255), (141, 110, 99, 255), (188, 150, 130, 255)]
SNOW     = [(255, 255, 255, 255), (236, 239, 241, 255), (207, 216, 220, 255)]
LAVA     = [(191, 54, 12, 255), (230, 74, 25, 255), (255, 152, 0, 255), (255, 235, 59, 255)]
TUNDRA   = [(80, 90, 60, 255), (121, 134, 100, 255), (158, 168, 130, 255), (207, 216, 220, 255)]
ICE      = [(144, 202, 249, 255), (187, 222, 251, 255), (224, 242, 254, 255), (235, 248, 255, 255)]
MEADOW   = [(56, 142, 60, 255), (102, 187, 106, 255), (139, 195, 74, 255)]
DESERT   = [(200, 140, 60, 255), (230, 170, 90, 255), (255, 204, 128, 255), (255, 224, 178, 255)]
CLAY     = [(120, 75, 50, 255), (160, 100, 70, 255), (190, 130, 100, 255), (180, 90, 60, 255)]

TERRAIN_GENERATORS = [
    ("grass",        make_terrain("grass",        GRASS,    0)),
    ("water",        make_terrain("water",        WATER,    1)),
    ("deep_water",   make_terrain("deep_water",   DEEP_H2O, 2)),
    ("sand",         make_terrain("sand",         SAND,     3)),
    ("forest_floor", make_terrain("forest_floor", FOREST,   4)),
    ("swamp",        make_terrain("swamp",        SWAMP,    5)),
    ("stone_ground", make_terrain("stone_ground", STONE,    6)),
    ("dirt",         make_terrain("dirt",         DIRT,     7)),
    ("snow",         make_terrain("snow",         SNOW,     8)),
    ("lava",         make_terrain("lava",         LAVA,     9)),
    ("tundra",       make_terrain("tundra",       TUNDRA,   10)),
    ("ice",          make_terrain("ice",          ICE,      11)),
    ("meadow",       make_terrain("meadow",       MEADOW,   12)),
    ("desert",       make_terrain("desert",       DESERT,   13)),
    ("clay",         make_terrain("clay",         CLAY,     14)),
]

# sill keep the old seeded_rand for backward compatibility in Step 1
def seeded_rand(seed):
    """Deterministic random based on coordinates."""
    def f(x, y, offset=0):
        h = (x * 374761393 + y * 668265263 + offset * 1274126177) & 0xFFFFFFFF
        h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
        return ((h ^ (h >> 16)) & 0xFFFF) / 65536.0
    return f



def regenerate_terrain():
    """Regenerate all terrain tiles with procedural pixel art."""
    print("Regenerating terrain tiles...")
    terrain_dir = os.path.join(OUTPUT_DIR, "terrain")
    os.makedirs(terrain_dir, exist_ok=True)

    for name, gen_func in TERRAIN_GENERATORS:
        seed = hash(name) & 0xFFFF
        sr = seeded_rand(seed)
        img = gen_func(sr, seed)

        # Save 32x32
        path = os.path.join(terrain_dir, f"{name}.png")
        img.save(path)

        # Save preview (4x)
        preview = img.resize((128, 128), Image.NEAREST)
        preview_path = os.path.join(terrain_dir, f"{name}_preview.png")
        preview.save(preview_path)

        print(f"  Regenerated: terrain/{name}.png")

    # Update description files
    descriptions = {
        "grass": "草地 - 绿色草地的像素艺术瓦片",
        "water": "浅水 - 带有波纹图案的浅蓝色水面",
        "deep_water": "深海 - 深蓝色的海洋水域",
        "sand": "沙地 - 浅黄色的沙滩地面",
        "forest_floor": "森林 - 深绿色的森林地表",
        "swamp": "沼泽 - 沼泽泥泞的绿色水面",
        "stone_ground": "岩石 - 灰色的岩石地面",
        "dirt": "泥土 - 棕色的泥土地面",
        "snow": "雪地 - 白色积雪地面",
        "lava": "熔岩 - 橙红色的发光熔岩",
        "tundra": "冻原 - 棕绿色的冻原地表",
        "ice": "冰原 - 浅蓝色的冰冻表面",
        "meadow": "草甸 - 带有小花的茂密草地",
        "desert": "沙漠 - 橙色的干旱沙漠",
        "clay": "黏土 - 红棕色的黏土地面",
    }

    for name, desc in descriptions.items():
        desc_path = os.path.join(terrain_dir, f"{name}.txt")
        with open(desc_path, "w", encoding="utf-8") as f:
            f.write(f"{name} - {desc}\n\n"
                    f"类别: 地形\n"
                    f"生成方式: 程序化像素艺术\n"
                    f"像素尺寸: 32x32\n"
                    f"说明: {desc}。程序化生成的像素艺术地形瓦片，无缝平铺。")

    print(f"Terrain regeneration complete ({len(TERRAIN_GENERATORS)} tiles)\n")


# ===========================================================================
# Run
# ===========================================================================
if __name__ == "__main__":
    print("=" * 50)
    print("Step 1: Fixing transparency on object sprites...")
    print("=" * 50)
    fixed = fix_object_transparency()

    print("=" * 50)
    print("Step 2: Regenerating terrain with procedural pixel art...")
    print("=" * 50)
    regenerate_terrain()

    print("=" * 50)
    print("Done!")
    print(f"  - Fixed transparency: {fixed} sprites")
    print(f"  - Regenerated terrain: {len(TERRAIN_GENERATORS)} tiles")
    print("=" * 50)
