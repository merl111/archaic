import tempfile
import unittest
from pathlib import Path
from release_metadata import metadata


class ReleaseMetadata(unittest.TestCase):
    def test_pin_and_tag_validation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Cargo.toml").write_text('[workspace.package]\nversion="0.1.0"\nrust-version="1.96"\n')
            (root / "cui-revision").write_text("a" * 40)
            self.assertEqual(metadata(root, "v0.1.0"), {"tag": "v0.1.0", "cui": "a" * 40, "rust": "1.96.0"})
            for tag in ("v0.2.0", "v0.1.0\ninjected=true", "--draft"):
                with self.assertRaises(ValueError):
                    metadata(root, tag)
            (root / "cui-revision").write_text("main")
            with self.assertRaises(ValueError):
                metadata(root, "v0.1.0")


if __name__ == "__main__":
    unittest.main()
