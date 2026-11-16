#!/usr/bin/env python3
"""Writes a cursor theme whose every cursor is one transparent pixel.

cage draws its own pointer from the moment it starts, before the page's `cursor: none`
applies; pointing XCURSOR_PATH at this theme makes that pointer invisible too.
Usage: blank-cursor.py <dir>   (creates <dir>/default/cursors/…)
"""
import os
import struct
import sys

IMAGE = 0xFFFD0002


def xcursor():
    # File header, one table-of-contents entry, one 1×1 image chunk; all little-endian.
    header = b"Xcur" + struct.pack("<III", 16, 0x10000, 1)
    toc = struct.pack("<III", IMAGE, 1, 28)
    chunk = struct.pack("<IIIIIIIII", 36, IMAGE, 1, 1, 1, 1, 0, 0, 0) + struct.pack("<I", 0)
    return header + toc + chunk


def main():
    cursors = os.path.join(sys.argv[1], "default", "cursors")
    os.makedirs(cursors, exist_ok=True)
    with open(os.path.join(cursors, "default"), "wb") as f:
        f.write(xcursor())
    for name in ("left_ptr", "pointer", "hand2", "text", "xterm", "wait", "watch", "progress", "grabbing"):
        link = os.path.join(cursors, name)
        if not os.path.lexists(link):
            os.symlink("default", link)


if __name__ == "__main__":
    main()
