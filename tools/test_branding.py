"""Validate committed desktop icon sizes and container formats without Pillow."""
from pathlib import Path
import struct
import unittest

BRAND = Path(__file__).resolve().parents[1] / "assets/branding"


class Branding(unittest.TestCase):
    def test_png_and_embedded_pixels(self):
        for size in (16, 24, 32, 48, 64, 128, 256, 512, 1024):
            data = (BRAND / f"icon-{size}.png").read_bytes()
            self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
            self.assertEqual(struct.unpack(">II", data[16:24]), (size, size))
        for size in (32, 256):
            pixels = (BRAND / f"icon-{size}.rgba").read_bytes()
            self.assertEqual(len(pixels), size * size * 4)
            self.assertEqual(min(pixels[3::4]), 0)
            self.assertEqual(max(pixels[3::4]), 255)

    def test_windows_multi_resolution_directory(self):
        data = (BRAND / "archaic.ico").read_bytes()
        reserved, kind, count = struct.unpack_from("<HHH", data)
        self.assertEqual((reserved, kind), (0, 1))
        sizes = set()
        for index in range(count):
            w, h, _, _, _, _, length, offset = struct.unpack_from("<BBBBHHII", data, 6 + index * 16)
            sizes.add((w or 256, h or 256))
            self.assertLessEqual(offset + length, len(data))
            self.assertGreater(length, 0)
        self.assertEqual(sizes, {(s, s) for s in (16, 24, 32, 48, 64, 128, 256)})

    def test_mac_icon_container(self):
        data = (BRAND / "archaic.icns").read_bytes()
        self.assertEqual(data[:4], b"icns")
        self.assertEqual(struct.unpack_from(">I", data, 4)[0], len(data))
        offset, types = 8, set()
        while offset < len(data):
            kind, length = struct.unpack_from(">4sI", data, offset)
            self.assertGreater(length, 8)
            self.assertLessEqual(offset + length, len(data))
            types.add(kind)
            offset += length
        self.assertEqual(offset, len(data))
        self.assertTrue({b"ic07", b"ic08", b"ic09", b"ic10"}.issubset(types))


if __name__ == "__main__":
    unittest.main()
