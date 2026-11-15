#!/usr/bin/env python3
"""Export one trained M0 `.npz` to the compact Rust inference format.

Layout (little-endian):
  magic "TDM0" (4 bytes)
  version u8 (=1)
  hash_id u8 (1 = FNV-1a)
  reserved u16 (=0)
  dim u32
  hidden u32
  w1 f32[dim * hidden]  (row-major, dim rows)
  b1 f32[hidden]
  w2 f32[hidden]
  b2 f32[1]
"""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import numpy as np

MAGIC = b"TDM0"
VERSION = 1
HASH_FNV = 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", help="m0-tags-*.npz")
    parser.add_argument("output", help="destination .bin")
    args = parser.parse_args()

    with np.load(args.input) as data:
        dim = int(data["dim"])
        hidden = int(data["hidden"])
        w1 = np.asarray(data["w1"], dtype=np.float32).reshape(dim, hidden)
        b1 = np.asarray(data["b1"], dtype=np.float32).reshape(hidden)
        w2 = np.asarray(data["w2"], dtype=np.float32).reshape(hidden)
        b2 = np.asarray(data["b2"], dtype=np.float32).reshape(1)

    path = Path(args.output)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as handle:
        handle.write(MAGIC)
        handle.write(struct.pack("<BBHII", VERSION, HASH_FNV, 0, dim, hidden))
        handle.write(w1.tobytes(order="C"))
        handle.write(b1.tobytes(order="C"))
        handle.write(w2.tobytes(order="C"))
        handle.write(b2.tobytes(order="C"))
    print(f"wrote {path} ({path.stat().st_size} bytes, dim={dim}, hidden={hidden})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
