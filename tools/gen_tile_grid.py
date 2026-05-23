"""Generate a labelled reference grid of all tiles using Pillow."""
import os
from PIL import Image, ImageDraw, ImageFont

tiles_dir = "assets/kenney_tiny-town/Tiles"
out_path = "assets/kenney_tiny-town/tile_reference.png"

tile_w, tile_h = 16, 16
cols, rows = 12, 11
gap = 2
pad = 4

total_w = pad * 2 + cols * tile_w + (cols - 1) * gap
total_h = pad * 2 + rows * tile_h + (rows - 1) * gap

# Scale up for readability
scale = 4
canvas = Image.new("RGBA", (total_w * scale, total_h * scale), (0, 0, 0, 0))
draw = ImageDraw.Draw(canvas)

try:
    font = ImageFont.truetype("arial.ttf", 10)
except:
    font = ImageFont.load_default()

for idx in range(132):
    path = os.path.join(tiles_dir, f"tile_{idx:04d}.png")
    if not os.path.exists(path):
        continue
    tile = Image.open(path).convert("RGBA")
    col = idx % cols
    row = idx // cols
    x = (pad + col * (tile_w + gap)) * scale
    y = (pad + row * (tile_h + gap)) * scale
    tile_scaled = tile.resize((tile_w * scale, tile_h * scale), Image.NEAREST)
    canvas.paste(tile_scaled, (x, y), tile_scaled)

    # Draw index number
    draw.text((x + 2, y + 2), str(idx), fill=(255, 255, 255, 220), font=font)

canvas.save(out_path)
print(f"Saved {out_path} ({canvas.width}x{canvas.height})")
