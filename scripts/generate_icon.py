#!/usr/bin/env python3
"""Generate our geometric, original app icon without image-library dependencies."""
from pathlib import Path
import struct
import zlib

def inside(x, y, polygon):
    result = False
    j = len(polygon) - 1
    for i, (xi, yi) in enumerate(polygon):
        xj, yj = polygon[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            result = not result
        j = i
    return result

def png(size, tray=False):
    forward = [(48,73),(151,73),(151,49),(205,96),(151,143),(151,119),(48,119)]
    back = [(208,183),(105,183),(105,207),(51,160),(105,113),(105,137),(208,137)]
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            px, py = (x+.5)*256/size, (y+.5)*256/size
            arrow = inside(px,py,forward) or inside(px,py,back)
            corner = max(0, 50-px, px-206)**2 + max(0,50-py,py-206)**2 <= 43**2
            if tray:
                color = (0,0,0,255 if arrow else 0)
            else:
                color = (134,231,197,255) if arrow else (22,43,48,255 if corner else 0)
            rows.extend(color)
    def chunk(kind,data):
        return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',size,size,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(rows))+chunk(b'IEND',b'')

root=Path(__file__).resolve().parents[1]/'src-tauri'/'icons'
root.mkdir(exist_ok=True)
(root/'app-icon.png').write_bytes(png(1024))
(root/'tray.png').write_bytes(png(64,True))
