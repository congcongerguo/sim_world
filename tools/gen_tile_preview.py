"""Generate a visual reference grid of all Kenney Tiny Town tiles."""
import struct, zlib, os, sys

def read_png_pixels(path):
    """Read RGBA pixels from a PNG file."""
    with open(path, 'rb') as f:
        f.read(8)  # signature
        pixels = None
        width = height = 0
        while True:
            length = struct.unpack('>I', f.read(4))[0]
            chunk_type = f.read(4)
            data = f.read(length)
            f.read(4)  # CRC
            if chunk_type == b'IHDR':
                width, height = struct.unpack('>II', data[:8])
            elif chunk_type == b'IDAT':
                decompressed = zlib.decompress(data)
                if pixels is None:
                    pixels = bytearray()
                pixels.extend(decompressed)
            elif chunk_type == b'IEND':
                break

    # Convert from raw scanlines to RGBA
    rgba = bytearray(width * height * 4)
    src = 0
    stride = width * 4 + 1  # +1 for filter byte per row
    for y in range(height):
        filter_byte = pixels[src]
        src += 1
        for x in range(width):
            if filter_byte == 0:  # None
                r, g, b, a = pixels[src:src+4]
            elif filter_byte == 2:  # Up - need previous row
                r = (pixels[src] + rgba[(y-1)*width*4 + x*4]) % 256
                g = (pixels[src+1] + rgba[(y-1)*width*4 + x*4 + 1]) % 256
                b = (pixels[src+2] + rgba[(y-1)*width*4 + x*4 + 2]) % 256
                a = (pixels[src+3] + rgba[(y-1)*width*4 + x*4 + 3]) % 256
            else:
                r, g, b, a = pixels[src:src+4]
            idx = y * width * 4 + x * 4
            rgba[idx] = r
            rgba[idx+1] = g
            rgba[idx+2] = b
            rgba[idx+3] = a
            src += 4
    return width, height, bytes(rgba)

def write_png(path, width, height, rgba_pixels):
    """Write RGBA pixels as PNG."""
    import struct, zlib
    def chunk(ctype, data):
        c = ctype + data
        crc = struct.pack('>I', zlib.crc32(c) & 0xffffffff)
        return struct.pack('>I', len(data)) + c + crc

    raw = bytearray()
    for y in range(height):
        raw.append(0)  # filter: None
        raw.extend(rgba_pixels[y * width * 4:(y + 1) * width * 4])

    ihdr = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    idat_data = zlib.compress(bytes(raw))

    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n')
        f.write(chunk(b'IHDR', ihdr))
        f.write(chunk(b'IDAT', idat_data))
        f.write(chunk(b'IEND', b''))

def main():
    tiles_dir = os.path.join('assets', 'kenney_tiny-town', 'Tiles')
    out_path = os.path.join('assets', 'kenney_tiny-town', 'tile_reference.png')

    tile_w, tile_h = 16, 16
    cols, rows = 12, 11
    gap = 2
    pad = 4

    total_w = pad * 2 + cols * (tile_w + gap) - gap
    total_h = pad * 2 + rows * (tile_h + gap) - gap

    # Find all tile files
    tiles = []
    for i in range(132):
        path = os.path.join(tiles_dir, f'tile_{i:04d}.png')
        if os.path.exists(path):
            tiles.append(path)
        else:
            tiles.append(None)

    # Create output canvas (RGBA)
    canvas = bytearray(total_w * total_h * 4)

    for idx, tile_path in enumerate(tiles):
        if tile_path is None:
            continue
        col = idx % cols
        row = idx // cols
        try:
            w, h, px = read_png_pixels(tile_path)
            x0 = pad + col * (tile_w + gap)
            y0 = pad + row * (tile_h + gap)
            for y in range(min(h, tile_h)):
                for x in range(min(w, tile_w)):
                    src_idx = (y * w + x) * 4
                    dst_idx = ((y0 + y) * total_w + (x0 + x)) * 4
                    canvas[dst_idx:dst_idx+4] = px[src_idx:src_idx+4]
        except Exception as e:
            print(f"Error processing {tile_path}: {e}", file=sys.stderr)

    write_png(out_path, total_w, total_h, bytes(canvas))
    print(f"Wrote {out_path} ({total_w}x{total_h})")

if __name__ == '__main__':
    main()
