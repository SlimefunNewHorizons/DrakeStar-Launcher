"""Ensure the GUI harness does not report a process that died during capture."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import MagicMock, patch


spec = importlib.util.spec_from_file_location(
    "gui_probe", Path(__file__).with_name("gui-probe.py"))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class CaptureLifecycleTest(unittest.TestCase):
    def run_probe(self, directory, *, exit_during_capture):
        manager, app = MagicMock(), MagicMock()
        app.pid = 12345
        app.poll.return_value = None
        app.returncode = 3

        def run(arguments, **kwargs):
            if arguments[0] == "import" and exit_during_capture:
                app.poll.return_value = 3
            return subprocess.CompletedProcess(arguments, 0, "42\n", "")

        with (patch.object(probe.sys, "argv", ["gui-probe.py", "launcher", directory]),
              patch.object(probe.subprocess, "Popen", side_effect=[manager, app]),
              patch.object(probe.subprocess, "run", side_effect=run),
              patch.object(probe.subprocess, "check_output", return_value="DrakeStar\n"),
              patch.object(probe.time, "sleep"),
              patch.object(probe.os, "killpg") as terminate):
            try:
                probe.main()
            finally:
                terminate.assert_called_once_with(app.pid, probe.signal.SIGTERM)
                app.wait.assert_called_once_with(timeout=5)
                manager.terminate.assert_called_once()

    def test_exit_during_capture_cannot_write_success_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "exited during capture"):
                self.run_probe(directory, exit_during_capture=True)
            self.assertFalse((Path(directory) / "gui.json").exists())

    def test_live_process_records_capture_and_still_gets_cleaned_up(self):
        with tempfile.TemporaryDirectory() as directory:
            self.run_probe(directory, exit_during_capture=False)
            report = json.loads((Path(directory) / "gui.json").read_text())
            self.assertTrue(report["alive_after_capture"])
            self.assertTrue(report["render_requires_visual_review"])


if __name__ == "__main__":
    unittest.main()
