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


class OnboardingTest(unittest.TestCase):
    def exercise(self, directory, *, initial=None, persist=True, exit_during_capture=False):
        data = Path(directory)
        settings = data / "settings.json"
        settings.write_text(json.dumps(initial if initial is not None else {
            "first_run_done": False, "send_reports": True, "device_id": "synthetic"}))
        app = MagicMock()
        app.poll.return_value = None

        def run(arguments, **kwargs):
            if arguments[:2] == ["xdotool", "mousemove"] and persist:
                settings.write_text(json.dumps({"first_run_done": True,
                                                 "send_reports": False,
                                                 "device_id": "synthetic"}))
            if arguments[0] == "import" and exit_during_capture:
                app.poll.return_value = 3
            return subprocess.CompletedProcess(arguments, 0)

        with (patch.object(probe.subprocess, "run", side_effect=run) as commands,
              patch.object(probe.time, "sleep"),
              patch.object(probe.time, "monotonic", side_effect=[0, 0, 9])):
            try:
                return probe.complete_onboarding(app, "42", data, data)
            finally:
                if initial is not None:
                    commands.assert_not_called()

    def test_ui_must_write_both_settings_without_exporting_identifier(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(self.exercise(directory), {
                "settings_saved_by_ui": True, "first_run_done": True,
                "send_reports": False})

    def test_missing_ipc_write_fails_even_when_launcher_is_alive(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "did not persist"):
                self.exercise(directory, persist=False)

    def test_preexisting_success_is_rejected_before_ui_actions(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "fresh default settings"):
                self.exercise(directory, initial={"first_run_done": True,
                                                  "send_reports": False})

    def test_exit_during_onboarding_capture_cannot_report_success(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "exited during onboarding capture"):
                self.exercise(directory, exit_during_capture=True)


if __name__ == "__main__":
    unittest.main()
