"""Reject wrong packages, downgrades, and unverifiable RPM payloads."""
import hashlib
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import package_backend as packages


class PackageBackendTest(unittest.TestCase):
    def test_mixed_or_unknown_formats_are_rejected_before_commands(self):
        with patch.object(packages, "command") as command:
            for older, newer in (("a.deb", "b.rpm"), ("a.zip", "b.zip")):
                with self.assertRaises(ValueError):
                    packages.select_backend(Path(older), Path(newer))
            command.assert_not_called()

    def test_debian_metadata_and_upgrade_keep_native_checks(self):
        backend = packages.select_backend(Path("old.deb"), Path("new.deb"))
        with patch.object(packages, "command", side_effect=["drake-star", "amd64", "0.6.0"]):
            self.assertEqual(backend.version(Path("new.deb")), "0.6.0")
        with patch.object(packages.subprocess, "run") as run:
            backend.require_upgrade("0.5.2", "0.6.0")
            run.assert_called_once_with(["dpkg", "--compare-versions", "0.5.2",
                                         "lt", "0.6.0"], check=True, timeout=5)

    def test_rpm_rejects_wrong_identity_or_architecture(self):
        backend = packages.Fedora()
        for metadata in ("other\nx86_64\n0:0.6.0-1", "drake-star\naarch64\n0:0.6.0-1"):
            with patch.object(packages, "command", return_value=metadata):
                with self.assertRaisesRegex(ValueError, "drake-star x86_64"):
                    backend.version(Path("new.rpm"))

    def test_rpm_metadata_preserves_epoch_and_release(self):
        with patch.object(packages, "command", return_value="drake-star\nx86_64\n2:0.6.0-3"):
            self.assertEqual(packages.Fedora().version(Path("new.rpm")), "2:0.6.0-3")

    def test_rpm_native_ordering_rejects_equal_and_downgrade(self):
        for comparison in (-1, 0, 1):
            calls = []

            def compare(older, newer):
                calls.append((older, newer))
                return comparison

            with patch.dict(sys.modules, rpm=SimpleNamespace(labelCompare=compare)):
                if comparison < 0:
                    packages.Fedora().require_upgrade("0:0.9.0-1", "0:0.10.0-1")
                else:
                    with self.assertRaisesRegex(ValueError, "strictly increasing"):
                        packages.Fedora().require_upgrade("0:0.9.0-1", "0:0.10.0-1")
            self.assertEqual(calls, [(("0", "0.9.0", "1"), ("0", "0.10.0", "1"))])

    def test_rpm_verification_failure_propagates(self):
        error = subprocess.CalledProcessError(1, ["rpm", "--verify"])
        with patch.object(packages, "command", side_effect=["installed", error]) as command:
            with self.assertRaises(subprocess.CalledProcessError):
                packages.Fedora().install(Path("new.rpm"))
            self.assertEqual(command.call_args_list[0].args, ("rpm", "--upgrade", "new.rpm"))

    def test_rpm_missing_executable_cannot_report_digest(self):
        with patch.object(packages.subprocess, "run"):
            with self.assertRaisesRegex(ValueError, "missing from payload"):
                packages.Fedora().payload_digest(Path("new.rpm"))

    def test_rpm_extracts_only_executable_to_stdout(self):
        calls = []

        def run(arguments, **kwargs):
            calls.append(arguments)
            kwargs["stdout"].write(b"archive" if arguments[0] == "rpm2cpio" else b"ELF fixture")

        with patch.object(packages.subprocess, "run", side_effect=run):
            self.assertEqual(packages.Fedora().payload_digest(Path("new.rpm")),
                             hashlib.sha256(b"ELF fixture").hexdigest())
        self.assertEqual(calls[1], ["cpio", "--extract", "--to-stdout",
                                   "./usr/bin/drakestar-launcher"])


if __name__ == "__main__":
    unittest.main()
