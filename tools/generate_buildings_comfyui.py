"""
ComfyUI SDXL -> 像素建筑精灵图生成 (v4 - 1024x1024 + 众数滤波)
"""
import os, sys, json, time, uuid, io
import urllib.request, urllib.error
from PIL import Image
from collections import Counter, deque

API_URL = "http://192.168.0.106:8000"
OUTPUT_DIR = "D:/work/rust_len/sim_world/assets/pixel_prototypes"
CLIENT_ID = str(uuid.uuid4())

# ===========================================================================
# 建筑 Prompt 配置 (使用更精准的 prompt)
# ===========================================================================

BUILDING_CONFIGS = [
    {
        "name": "house", "category": "buildings", "seed": 342,
        "prompt": (
            "pixel art game sprite of a cozy wooden house, "
            "brown wood walls, dark brown shingle roof, stone chimney, "
            "warm glowing window, wooden front door, centered, "
            "flat front view, game asset, on white background"
        ),
    },
    {
        "name": "stone_house", "category": "buildings", "seed": 343,
        "prompt": (
            "pixel art game sprite of a stone masonry house, "
            "grey stone block walls, dark slate roof, wooden door, "
            "small window, centered, "
            "flat front view, game asset, on white background"
        ),
    },
    {
        "name": "watchtower", "category": "buildings", "seed": 344,
        "prompt": (
            "pixel art game sprite of a tall wooden watchtower, "
            "elevated lookout platform, pointed roof, wooden ladder, "
            "centered, flat front view, game asset, on white background"
        ),
    },
    {
        "name": "workshop", "category": "buildings", "seed": 345,
        "prompt": (
            "pixel art game sprite of a blacksmith workshop, "
            "large wooden building, chimney, wide open double doors, "
            "anvil inside, centered, "
            "flat front view, game asset, on white background"
        ),
    },
    {
        "name": "well", "category": "buildings", "seed": 346,
        "prompt": (
            "pixel art game sprite of a stone well, "
            "round stone base, wooden roof on posts, bucket on rope, "
            "centered, flat front view, game asset, on white background"
        ),
    },
    {
        "name": "shop", "category": "misc", "seed": 347,
        "prompt": (
            "pixel art game sprite of a medieval market shop, "
            "red striped awning, wooden signboard, display window, "
            "open front door, centered, "
            "flat front view, game asset, on white background"
        ),
    },
]

NEGATIVE_PROMPT = (
    "photorealistic, 3d render, blurry, low quality, distorted, "
    "ugly, deformed, messy, noisy, watermark, text, signature, "
    "complex background, people, animals, multiple views"
)

# ===========================================================================
# ComfyUI API (1024x1024 native SDXL resolution)
# ===========================================================================

def build_workflow(prompt: str, negative: str, seed: int) -> dict:
    return {
        "3": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": "sd_xl_base_1.0.safetensors"}},
        "4": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["3", 1]}},
        "5": {"class_type": "CLIPTextEncode", "inputs": {"text": negative, "clip": ["3", 1]}},
        "6": {"class_type": "EmptyLatentImage", "inputs": {"width": 1024, "height": 1024, "batch_size": 1}},
        "7": {"class_type": "KSampler", "inputs": {
            "model": ["3", 0], "positive": ["4", 0], "negative": ["5", 0],
            "latent_image": ["6", 0], "seed": seed, "steps": 25, "cfg": 7.0,
            "sampler_name": "euler", "scheduler": "karras", "denoise": 1.0,
        }},
        "8": {"class_type": "VAEDecode", "inputs": {"samples": ["7", 0], "vae": ["3", 2]}},
        "9": {"class_type": "SaveImage", "inputs": {"images": ["8", 0], "filename_prefix": "bld"}},
    }

def queue_prompt(workflow: dict) -> str:
    data = json.dumps({"prompt": workflow, "client_id": CLIENT_ID}).encode("utf-8")
    return json.loads(urllib.request.urlopen(
        urllib.request.Request(f"{API_URL}/prompt", data=data, headers={"Content-Type": "application/json"})
    ).read())["prompt_id"]

def wait_for_result(prompt_id: str, timeout: int = 180) -> list[dict]:
    for _ in range(timeout * 2):
        time.sleep(0.5)
        try:
            history = json.loads(urllib.request.urlopen(
                urllib.request.Request(f"{API_URL}/history/{prompt_id}")
            ).read())
        except urllib.error.HTTPError:
            continue
        if prompt_id not in history:
            continue
        images = []
        for node_out in history[prompt_id].get("outputs", {}).values():
            images.extend(node_out.get("images", []))
        if images:
            return images
    raise TimeoutError(f"Prompt {prompt_id} timeout")

def download_image(filename: str, subfolder: str = "") -> bytes:
    url = f"{API_URL}/view?filename={filename}"
    if subfolder:
        url += f"&subfolder={subfolder}"
    return urllib.request.urlopen(urllib.request.Request(url)).read()

# ===========================================================================
# 像素画处理管线 (众数滤波降采样)
# ===========================================================================

BLOCK_SIZE = 32   # 1024/32 = 32 -> 每个输出像素从 32x32 区域取众数

def remove_background(img: Image.Image, tolerance: int = 60) -> Image.Image:
    """Flood-fill 从边缘移除白色/浅色背景."""
    img = img.convert("RGBA")
    px = img.load()
    w, h = img.size

    # 从四角检测背景色
    corners = [px[0,0], px[w-1,0], px[0,h-1], px[w-1,h-1]]
    bg_r = sum(c[0] for c in corners) // 4
    bg_g = sum(c[1] for c in corners) // 4
    bg_b = sum(c[2] for c in corners) // 4

    mask = [[False] * w for _ in range(h)]
    dq = deque()

    def try_add(y, x):
        if 0 <= y < h and 0 <= x < w and not mask[y][x]:
            r, g, b = px[x, y][:3]
            if abs(r - bg_r) + abs(g - bg_g) + abs(b - bg_b) < tolerance * 3:
                dq.append((y, x))

    # 从四边种子填充
    for x in range(w):
        try_add(0, x)
        try_add(h-1, x)
    for y in range(1, h-1):
        try_add(y, 0)
        try_add(y, w-1)

    while dq:
        cy, cx = dq.popleft()
        if mask[cy][cx]:
            continue
        mask[cy][cx] = True
        for ny, nx in [(cy-1,cx),(cy+1,cx),(cy,cx-1),(cy,cx+1)]:
            try_add(ny, nx)

    for y in range(h):
        for x in range(w):
            if mask[y][x]:
                px[x, y] = (0, 0, 0, 0)

    return img


def mode_downscale(img: Image.Image, target_size: int = 32) -> Image.Image:
    """
    众数滤波降采样.
    每个输出像素取对应输入区域中出现最多的颜色.
    使用粗略量化对颜色分组后再计众数.
    """
    px = img.load()
    w, h = img.size
    step = w // target_size   # e.g., 1024/32 = 32

    out = Image.new("RGBA", (target_size, target_size))
    opx = out.load()

    for oy in range(target_size):
        for ox in range(target_size):
            colors = []
            for dy in range(step):
                for dx in range(step):
                    ix = ox * step + dx
                    iy = oy * step + dy
                    r, g, b, a = px[ix, iy]
                    if a < 128:
                        colors.append(None)  # transparent
                    else:
                        # 32-level per channel grouping
                        colors.append((r // 32, g // 32, b // 32))

            most_common = Counter(colors).most_common(1)[0][0]
            if most_common is None:
                opx[ox, oy] = (0, 0, 0, 0)
            else:
                r, g, b = most_common
                opx[ox, oy] = (r * 32 + 16, g * 32 + 16, b * 32 + 16, 255)

    return out


def clean_isolated_pixels(img: Image.Image) -> Image.Image:
    """移除被 3+ 个透明像素包围的孤立像素."""
    px = img.load()
    w, h = img.size
    for y in range(1, h - 1):
        for x in range(1, w - 1):
            if px[x, y][3] == 0:
                continue
            tc = sum(1 for dx, dy in [(-1,0),(1,0),(0,-1),(0,1)] if px[x+dx, y+dy][3] == 0)
            if tc >= 3:
                px[x, y] = (0, 0, 0, 0)
    return img


def consolidate_colors(img: Image.Image, levels: int = 8) -> Image.Image:
    """
    整理颜色到更少的色阶.
    用于消除众数滤波分组引入的微小色差.
    """
    px = img.load()
    w, h = img.size
    step = 256 // levels
    offset = step // 2
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a > 0:
                px[x, y] = (
                    r // step * step + offset,
                    g // step * step + offset,
                    b // step * step + offset,
                    255,
                )
    return img


# 固定 16 色调色板 (游戏像素风格)
FIXED_PALETTE = [
    (0, 0, 0),
    (45, 28, 18),    # 1: 深棕 (阴影/轮廓)
    (74, 49, 32),    # 2: 暗棕 (屋顶)
    (120, 75, 45),   # 3: 中棕 (木墙)
    (175, 120, 65),  # 4: 浅棕 (装饰)
    (100, 90, 75),   # 5: 暗石
    (155, 145, 130), # 6: 中石
    (195, 185, 170), # 7: 浅石
    (40, 50, 35),    # 8: 暗绿
    (85, 110, 60),   # 9: 绿
    (180, 70, 45),   # A: 红 (烟囱)
    (200, 170, 90),  # B: 暖黄 (窗光)
    (135, 110, 85),  # C: 棕褐
    (220, 200, 170), # D: 奶油白 (高光)
    (60, 45, 35),    # E: 深木
    (65, 75, 80),    # F: 石板灰
]


def remap_to_palette(img: Image.Image) -> Image.Image:
    """将图像每个像素映射到固定调色板中最接近的颜色."""
    px = img.load()
    out = Image.new("RGBA", img.size)
    op = out.load()
    w, h = img.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a < 128:
                op[x, y] = (0, 0, 0, 0)
                continue
            best = 1
            best_d = 999999
            for i in range(1, len(FIXED_PALETTE)):
                pr, pg, pb = FIXED_PALETTE[i]
                d = (r - pr) ** 2 + (g - pg) ** 2 + (b - pb) ** 2
                if d < best_d:
                    best_d = d
                    best = i
            pr, pg, pb = FIXED_PALETTE[best]
            op[x, y] = (pr, pg, pb, 255)
    return out


def process_sprite(image_data: bytes, output_path: str):
    """完整管线: bg移除 -> 众数滤波降采样 -> 固定调色板映射."""
    img = Image.open(io.BytesIO(image_data)).convert("RGBA")
    w, h = img.size
    print(f"  源图: {w}x{h}")

    # 保存原始 SDXL 输出（含白色背景）
    sxl_path = output_path.replace(".png", "_sxl.png")
    os.makedirs(os.path.dirname(sxl_path), exist_ok=True)
    img.save(sxl_path)

    # 1) 背景移除
    img = remove_background(img, tolerance=60)

    # 2) 透明阈值: 清除抗锯齿边缘
    px = img.load()
    for y in range(h):
        for x in range(w):
            if 0 < px[x, y][3] < 200:
                px[x, y] = (0, 0, 0, 0)
            elif px[x, y][3] >= 200:
                px[x, y] = (px[x, y][0], px[x, y][1], px[x, y][2], 255)

    # 3) 众数滤波降采样到 32x32
    out = mode_downscale(img, 32)

    # 4) 映射到固定调色板 (清理颜色)
    out = remap_to_palette(out)

    # 5) 清理孤立像素
    out = clean_isolated_pixels(out)

    # 6) 保存
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    out.save(output_path)

    n_colors = len(set(p for p in out.getdata() if p[3] > 0))
    print(f"  -> {output_path} ({n_colors} 颜色)")

    # 7) Preview (4x)
    preview = out.resize((128, 128), Image.NEAREST)
    preview_path = output_path.replace(".png", "_preview.png")
    preview.save(preview_path)

    # 8) 全尺寸调试 (bg removed)
    debug_path = output_path.replace(".png", "_full.png")
    img.save(debug_path)


def write_description(cfg: dict):
    name = cfg["name"]
    cat = cfg["category"]
    path = os.path.join(OUTPUT_DIR, cat, f"{name}.txt")
    cn = {"house": "房屋", "stone_house": "石屋", "watchtower": "瞭望塔",
          "workshop": "工坊", "well": "水井", "shop": "商店"}.get(name, name)

    with open(path, "w", encoding="utf-8") as f:
        f.write(
            f"{name} - {cn}\n\n"
            f"类别: {'建筑 - 地图上的建筑物' if cat == 'buildings' else '杂项'}\n"
            f"生成引擎: ComfyUI SDXL (1024x1024) + 众数滤波降采样\n"
            f"种子: {cfg['seed']}\n"
            f"像素尺寸: 32x32 (预览: 128x128)\n\n"
            f"描述: 游戏的{cn}像素艺术原型 (v4)。\n"
            f"使用 SDXL 1024x1024 生成，众数滤波降采样到 32x32。\n\n"
        )


# ===========================================================================
# Main
# ===========================================================================

ASCII_CHARS = " .:-=+*#%@"

def print_ascii_preview(img: Image.Image):
    px = img.load()
    w, h = img.size
    for y in range(h):
        line = ""
        for x in range(w):
            r, g, b, a = px[x, y]
            if a == 0:
                line += " "
            else:
                gray = (r + g + b) // 3
                idx = gray * (len(ASCII_CHARS) - 1) // 255
                line += ASCII_CHARS[idx]
        print(line)


def main():
    print("=" * 60)
    print("ComfyUI SDXL 像素建筑生成 (v4 - 众数滤波)")
    print(f"API: {API_URL}")
    print(f"分辨率: 1024x1024 -> 众数滤波 32x32")
    print("=" * 60)

    for i, cfg in enumerate(BUILDING_CONFIGS):
        name = cfg["name"]
        cat = cfg["category"]
        output_path = os.path.join(OUTPUT_DIR, cat, f"{name}.png")

        print(f"\n[{i+1}/{len(BUILDING_CONFIGS)}] {name} (seed={cfg['seed']})...")

        workflow = build_workflow(cfg["prompt"], NEGATIVE_PROMPT, cfg["seed"])

        try:
            prompt_id = queue_prompt(workflow)
            print(f"  已队列: {prompt_id[:8]}...")
        except Exception as e:
            print(f"  !! 队列失败: {e}")
            continue

        try:
            images = wait_for_result(prompt_id)
        except Exception as e:
            print(f"  !! 等待失败: {e}")
            continue

        try:
            img_data = download_image(images[0]["filename"], images[0].get("subfolder", ""))
        except Exception as e:
            print(f"  !! 下载失败: {e}")
            continue

        try:
            process_sprite(img_data, output_path)
        except Exception as e:
            print(f"  !! 处理失败: {e}")
            import traceback; traceback.print_exc()
            continue

        write_description(cfg)

        # ASCII 预览
        img = Image.open(output_path).convert("RGBA")
        print(f"  ASCII 预览 (32x32):")
        print_ascii_preview(img)

    print(f"\n{'=' * 60}")
    print("完成!")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
