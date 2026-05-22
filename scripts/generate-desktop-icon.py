#!/usr/bin/env python3
"""Generate reproducible macOS desktop icons for the Tauri app.

The script intentionally uses only Python's standard library plus macOS
`sips`/`iconutil` when available, so regenerating icons does not depend on
Pillow, ImageMagick, or npm packages.
"""

from __future__ import annotations

import math
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ICONS_DIR = ROOT / "apps" / "desktop" / "src-tauri" / "icons"
ICON_PNG = ICONS_DIR / "icon.png"
ICON_ICNS = ICONS_DIR / "icon.icns"
CANVAS_SIZE = 512
SCALE = 3


Color = tuple[int, int, int, int]


def main() -> int:
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    image = render_icon(CANVAS_SIZE * SCALE)
    image = downsample(image, CANVAS_SIZE * SCALE, CANVAS_SIZE)
    write_png(ICON_PNG, CANVAS_SIZE, CANVAS_SIZE, image)
    generate_icns()
    print(f"generated {ICON_PNG.relative_to(ROOT)}")
    if ICON_ICNS.exists():
        print(f"generated {ICON_ICNS.relative_to(ROOT)}")
    return 0


def render_icon(size: int) -> bytearray:
    pixels = bytearray([0, 0, 0, 0] * size * size)

    def s(value: float) -> float:
        return value * size / CANVAS_SIZE

    for y in range(size):
        for x in range(size):
            nx = x / (size - 1)
            ny = y / (size - 1)
            bg = lerp_color((22, 29, 38, 255), (38, 47, 59, 255), ny)
            bg = lerp_color(bg, (23, 78, 113, 255), max(0.0, 1.0 - distance(nx, ny, 0.18, 0.16) * 3.2))
            bg = lerp_color(bg, (20, 112, 88, 255), max(0.0, 1.0 - distance(nx, ny, 0.82, 0.82) * 3.8))
            set_px(pixels, size, x, y, bg)

    mask_outside_rounded_rect(pixels, size, s(38), s(38), s(436), s(436), s(88))
    rounded_rect(pixels, size, s(76), s(96), s(360), s(280), s(34), (10, 15, 22, 180))
    rounded_rect(pixels, size, s(96), s(122), s(320), s(228), s(24), (20, 27, 36, 245))

    stroke_polyline(
        pixels,
        size,
        [(s(150), s(194)), (s(206), s(246)), (s(150), s(298))],
        s(24),
        (238, 244, 250, 255),
    )
    rounded_rect(pixels, size, s(236), s(279), s(100), s(24), s(12), (74, 170, 255, 255))
    rounded_rect(pixels, size, s(352), s(332), s(56), s(56), s(28), (41, 206, 128, 255))
    stroke_polyline(
        pixels,
        size,
        [(s(368), s(360)), (s(380), s(372)), (s(396), s(348))],
        s(8),
        (7, 44, 31, 255),
    )

    return pixels


def generate_icns() -> None:
    sips = shutil.which("sips")
    iconutil = shutil.which("iconutil")
    if not sips or not iconutil:
        return

    names = {
        16: "icon_16x16.png",
        32: "icon_16x16@2x.png",
        32.1: "icon_32x32.png",
        64: "icon_32x32@2x.png",
        128: "icon_128x128.png",
        256: "icon_128x128@2x.png",
        256.1: "icon_256x256.png",
        512: "icon_256x256@2x.png",
        512.1: "icon_512x512.png",
        1024: "icon_512x512@2x.png",
    }
    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "PriorityAgent.iconset"
        iconset.mkdir()
        for key, name in names.items():
            size = int(key)
            output = iconset / name
            subprocess.run(
                [sips, "-z", str(size), str(size), str(ICON_PNG), "--out", str(output)],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        subprocess.run(
            [iconutil, "-c", "icns", str(iconset), "-o", str(ICON_ICNS)],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )


def rounded_rect(
    pixels: bytearray,
    size: int,
    x: float,
    y: float,
    w: float,
    h: float,
    radius: float,
    color: Color,
    cutout: bool = False,
) -> None:
    for py in range(max(0, int(y)), min(size, math.ceil(y + h))):
        for px in range(max(0, int(x)), min(size, math.ceil(x + w))):
            dx = max(x - px, 0, px - (x + w - 1))
            dy = max(y - py, 0, py - (y + h - 1))
            inner_x = min(abs(px - x), abs(px - (x + w - 1)))
            inner_y = min(abs(py - y), abs(py - (y + h - 1)))
            corner = inner_x < radius and inner_y < radius
            if corner:
                cx = x + radius if px < x + radius else x + w - radius
                cy = y + radius if py < y + radius else y + h - radius
                alpha = smooth(radius + 1.0, radius - 1.0, math.hypot(px - cx, py - cy))
            else:
                alpha = 1.0 if dx <= 0 and dy <= 0 else 0.0
            if alpha <= 0:
                continue
            if cutout:
                existing = get_px(pixels, size, px, py)
                set_px(pixels, size, px, py, (existing[0], existing[1], existing[2], int(existing[3] * (1 - alpha))))
            else:
                blend_px(pixels, size, px, py, color, alpha)


def mask_outside_rounded_rect(
    pixels: bytearray,
    size: int,
    x: float,
    y: float,
    w: float,
    h: float,
    radius: float,
) -> None:
    for py in range(size):
        for px in range(size):
            if px < x or px >= x + w or py < y or py >= y + h:
                set_px(pixels, size, px, py, (0, 0, 0, 0))
                continue
            inner_x = min(abs(px - x), abs(px - (x + w - 1)))
            inner_y = min(abs(py - y), abs(py - (y + h - 1)))
            if inner_x >= radius or inner_y >= radius:
                continue
            cx = x + radius if px < x + radius else x + w - radius
            cy = y + radius if py < y + radius else y + h - radius
            alpha = smooth(radius + 1.0, radius - 1.0, math.hypot(px - cx, py - cy))
            if alpha >= 1:
                continue
            existing = get_px(pixels, size, px, py)
            set_px(
                pixels,
                size,
                px,
                py,
                (existing[0], existing[1], existing[2], int(existing[3] * alpha)),
            )


def stroke_polyline(
    pixels: bytearray,
    size: int,
    points: list[tuple[float, float]],
    width: float,
    color: Color,
) -> None:
    radius = width / 2
    min_x = max(0, int(min(p[0] for p in points) - width))
    max_x = min(size, math.ceil(max(p[0] for p in points) + width))
    min_y = max(0, int(min(p[1] for p in points) - width))
    max_y = min(size, math.ceil(max(p[1] for p in points) + width))
    for y in range(min_y, max_y):
        for x in range(min_x, max_x):
            d = min(
                distance_to_segment(x, y, points[index], points[index + 1])
                for index in range(len(points) - 1)
            )
            alpha = smooth(radius + 1.0, radius - 1.0, d)
            if alpha > 0:
                blend_px(pixels, size, x, y, color, alpha)


def downsample(pixels: bytearray, src_size: int, dst_size: int) -> bytearray:
    factor = src_size // dst_size
    output = bytearray([0, 0, 0, 0] * dst_size * dst_size)
    for y in range(dst_size):
        for x in range(dst_size):
            total = [0, 0, 0, 0]
            for sy in range(factor):
                for sx in range(factor):
                    color = get_px(pixels, src_size, x * factor + sx, y * factor + sy)
                    for index in range(4):
                        total[index] += color[index]
            count = factor * factor
            set_px(output, dst_size, x, y, tuple(value // count for value in total))  # type: ignore[arg-type]
    return output


def write_png(path: Path, width: int, height: int, pixels: bytearray) -> None:
    raw = bytearray()
    stride = width * 4
    for y in range(height):
        raw.append(0)
        raw.extend(pixels[y * stride : (y + 1) * stride])
    with path.open("wb") as handle:
        handle.write(b"\x89PNG\r\n\x1a\n")
        write_chunk(handle, b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        write_chunk(handle, b"IDAT", zlib.compress(bytes(raw), 9))
        write_chunk(handle, b"IEND", b"")


def write_chunk(handle, chunk_type: bytes, data: bytes) -> None:
    handle.write(struct.pack(">I", len(data)))
    handle.write(chunk_type)
    handle.write(data)
    checksum = zlib.crc32(chunk_type)
    checksum = zlib.crc32(data, checksum)
    handle.write(struct.pack(">I", checksum & 0xFFFFFFFF))


def get_px(pixels: bytearray, size: int, x: int, y: int) -> Color:
    index = (y * size + x) * 4
    return pixels[index], pixels[index + 1], pixels[index + 2], pixels[index + 3]


def set_px(pixels: bytearray, size: int, x: int, y: int, color: Color) -> None:
    index = (y * size + x) * 4
    pixels[index : index + 4] = bytes(color)


def blend_px(pixels: bytearray, size: int, x: int, y: int, color: Color, alpha: float) -> None:
    existing = get_px(pixels, size, x, y)
    source_alpha = (color[3] / 255) * alpha
    inverse = 1 - source_alpha
    out_alpha = source_alpha + existing[3] / 255 * inverse
    if out_alpha <= 0:
        set_px(pixels, size, x, y, (0, 0, 0, 0))
        return
    out = []
    for index in range(3):
        channel = (color[index] * source_alpha + existing[index] * (existing[3] / 255) * inverse) / out_alpha
        out.append(max(0, min(255, int(channel))))
    out.append(max(0, min(255, int(out_alpha * 255))))
    set_px(pixels, size, x, y, tuple(out))  # type: ignore[arg-type]


def lerp_color(a: Color, b: Color, t: float) -> Color:
    t = max(0.0, min(1.0, t))
    return tuple(int(a[index] + (b[index] - a[index]) * t) for index in range(4))  # type: ignore[return-value]


def distance(x1: float, y1: float, x2: float, y2: float) -> float:
    return math.hypot(x1 - x2, y1 - y2)


def distance_to_segment(px: float, py: float, a: tuple[float, float], b: tuple[float, float]) -> float:
    ax, ay = a
    bx, by = b
    dx = bx - ax
    dy = by - ay
    if dx == 0 and dy == 0:
        return math.hypot(px - ax, py - ay)
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


def smooth(edge0: float, edge1: float, value: float) -> float:
    if edge0 == edge1:
        return 0.0
    t = max(0.0, min(1.0, (value - edge0) / (edge1 - edge0)))
    return t * t * (3.0 - 2.0 * t)


if __name__ == "__main__":
    sys.exit(main())
