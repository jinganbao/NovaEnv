#!/usr/bin/env python3
"""生成 NovaEnv 应用图标（纯 Python 手写 PNG，无第三方依赖）。

用法: python3 scripts/gen_icons.py
输出: src-tauri/icons/{32x32.png, 128x128.png, 128x128@2x.png, icon.png}

图标设计: 品牌蓝背景 + 白色 "N" 字形（NovaEnv）。
正式发布前可用 `npm run tauri icon <源图>` 替换为正式设计图标。
"""
import os
import struct
import zlib


def make_png(size: int, pixel_fn) -> bytes:
    raw = bytearray()
    for y in range(size):
        raw.append(0)  # filter type: None
        for x in range(size):
            raw.extend(pixel_fn(x, y, size))

    def chunk(typ: bytes, data: bytes) -> bytes:
        out = struct.pack(">I", len(data)) + typ + data
        out += struct.pack(">I", zlib.crc32(typ + data) & 0xFFFFFFFF)
        return out

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(raw)))
        + chunk(b"IEND", b"")
    )


def _seg_dist(ux: float, uy: float, ax: float, ay: float, bx: float, by: float) -> float:
    dx, dy = bx - ax, by - ay
    length2 = dx * dx + dy * dy
    if length2 == 0:
        return ((ux - ax) ** 2 + (uy - ay) ** 2) ** 0.5
    t = ((ux - ax) * dx + (uy - ay) * dy) / length2
    t = max(0.0, min(1.0, t))
    px, py = ax + t * dx, ay + t * dy
    return ((ux - px) ** 2 + (uy - py) ** 2) ** 0.5


def pixel_n(x: int, y: int, size: int) -> tuple:
    u = x / size
    v = y / size
    r, g, b = 0x25, 0x63, 0xEB  # #2563eb 品牌蓝
    stroke = 0.075

    left_bar = 0.15 <= u <= 0.30 and 0.15 <= v <= 0.85
    right_bar = 0.70 <= u <= 0.85 and 0.15 <= v <= 0.85
    diagonal = _seg_dist(u, v, 0.30, 0.85, 0.70, 0.15) <= stroke

    if left_bar or right_bar or diagonal:
        r, g, b = 0xFF, 0xFF, 0xFF
    return (r, g, b, 255)


def main() -> None:
    out_dir = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")
    os.makedirs(out_dir, exist_ok=True)
    targets = [
        ("32x32.png", 32),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
        ("icon.png", 512),
    ]
    for name, size in targets:
        path = os.path.join(out_dir, name)
        with open(path, "wb") as f:
            f.write(make_png(size, pixel_n))
        print(f"generated {name} ({size}x{size})")


if __name__ == "__main__":
    main()
