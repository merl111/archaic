#!/usr/bin/env python3
"""Native installer format and command contracts; no platform signing credentials required."""
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import installer

class Installers(unittest.TestCase):
    def options(self):
        return SimpleNamespace(version="0.1.0",app_identity="Developer ID app",installer_identity="Developer ID installer",notary_profile="release-keychain",windows_certificate="a"*40,timestamp_url="https://timestamp.example.org")
    def stage(self,root):
        root.mkdir()
        for name in ["bin/archaic","archaic.exe","Archaic.app/Contents/MacOS/archaic","share/applications/org.archaic.desktop.desktop","README.txt","build.json","licenses/NOTICE"]:
            file=root/name; file.parent.mkdir(parents=True,exist_ok=True); file.write_text("fixture")
        (root/"bin/archaic").chmod(0o755)
        icon=root/"share/icons/hicolor/256x256/apps/org.archaic.desktop.png"
        icon.parent.mkdir(parents=True); icon.write_bytes(b"icon fixture")
        return root
    def test_debian_format_ownership_permissions_and_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); stage=self.stage(root/"stage"); out=root/"app.deb"
            (stage/"bin/archaic").chmod(0o600)  # Archive must supply target permissions.
            installer.deb(stage,out,"0.1.0","amd64","libgtk-4-1 (>= 4.6)","Maintainer <test@example.org>")
            raw=out.read_bytes(); self.assertEqual(raw[:8],b"!<arch>\n"); offset=8; members={}
            while offset<len(raw):
                header=raw[offset:offset+60]; self.assertEqual(header[-2:],b"`\n")
                size=int(header[48:58]); name=header[:16].decode().strip().rstrip("/")
                members[name]=raw[offset+60:offset+60+size]; offset+=60+size+size%2
            self.assertEqual(members["debian-binary"],b"2.0\n")
            with tarfile.open(fileobj=io.BytesIO(members["control.tar.gz"])) as archive:
                control=archive.extractfile("./control").read().decode()
                self.assertIn("Depends: libgtk-4-1 (>= 4.6)",control)
                self.assertIn("Architecture: amd64",control)
            with tarfile.open(fileobj=io.BytesIO(members["data.tar.gz"])) as archive:
                for entry in archive.getmembers(): self.assertEqual((entry.uid,entry.gid),(0,0))
                self.assertEqual(archive.getmember("./usr/bin/archaic").mode,0o755)
                self.assertEqual(archive.extractfile("./usr/share/doc/archaic/licenses/NOTICE").read(),b"fixture")
                self.assertEqual(archive.extractfile("./usr/share/icons/hicolor/256x256/apps/org.archaic.desktop.png").read(),b"icon fixture")
            with self.assertRaises(FileExistsError): installer.deb(stage,out,"0.1.0","amd64","libgtk-4-1","maintainer")
            with self.assertRaises(ValueError): installer.deb(stage,root/"bad.deb","0.1.0","amd64","libgtk\nInjected: bad","maintainer")
    def test_apple_sign_verify_package_notarize_staple_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); stage=self.stage(root/"stage"); calls=[]
            with patch.object(Path,"chmod",autospec=True,side_effect=Path.chmod) as chmod,patch.object(installer,"run",side_effect=lambda args:calls.append([str(a) for a in args])),patch.object(installer.subprocess,"check_output",return_value=b'{"status":"Accepted"}'):
                installer.native(stage,root/"app.pkg",self.options(),"Darwin")
            self.assertEqual([c[0] for c in calls],["codesign","codesign","pkgbuild","xcrun","xcrun"])
            self.assertIn("runtime",calls[0]); self.assertIn("--sign",calls[2]); self.assertIn("staple",calls[3])
            executable=stage/"Archaic.app/Contents/MacOS/archaic"
            chmod.assert_called_once_with(executable,0o755)
            if os.name != "nt":
                self.assertEqual(executable.stat().st_mode&0o777,0o755)
            with patch.object(installer,"run"),patch.object(installer.subprocess,"check_output",return_value=b'{"status":"Invalid"}'):
                with self.assertRaises(RuntimeError): installer.native(stage,root/"bad.pkg",self.options(),"Darwin")
    def test_windows_sign_both_binary_and_installer(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); stage=self.stage(root/"stage"); calls=[]
            with patch.object(installer,"run",side_effect=lambda args:calls.append([str(a) for a in args])):
                installer.native(stage,root/"setup.exe",self.options(),"Windows")
            self.assertEqual([c[:2] for c in calls],[["signtool","sign"],["signtool","verify"],["makensis","/V2"],["signtool","sign"],["signtool","verify"]])
            script=(installer.ROOT/"packaging/windows/archaic.nsi").read_text()
            self.assertIn('RequestExecutionLevel user',script)
            self.assertIn('--open $"%1$"',script)
            self.assertNotIn('RMDir /r "$INSTDIR"',script)

if __name__=="__main__": unittest.main()
