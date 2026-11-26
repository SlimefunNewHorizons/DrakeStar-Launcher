#!/usr/bin/env python3
"""Run as the fixture's unprivileged user inside dbus-run-session + Xvfb."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


def complete_onboarding(app, window, data, output):
    """Exercise the published 0.6.0 UI and require a real Rust settings write."""
    settings = data / "settings.json"
    initial = json.loads(settings.read_text())
    if initial.get("first_run_done") is not False or initial.get("send_reports") is not True:
        raise RuntimeError("Onboarding probe requires fresh default settings")
    subprocess.run(["xdotool", "windowactivate", "--sync", window],
                   check=True, timeout=5)
    # Coordinates belong only to the published 0.6.0 dialog at this fixed size.
    # Tab traversal would also reach background navigation (no focus trap).
    subprocess.run(["xdotool", "windowsize", "--sync", window, "1100", "720"],
                   check=True, timeout=5)
    subprocess.run(["xdotool", "mousemove", "--sync", "--window", window, "325", "443",
                    "click", "--clearmodifiers", "1", "sleep", "0.2",
                    "mousemove", "--sync", "--window", window, "550", "507",
                    "click", "--clearmodifiers", "1"], check=True, timeout=5)
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        if app.poll() is not None:
            raise RuntimeError("Launcher exited during onboarding")
        try:
            saved = json.loads(settings.read_text())
        except (FileNotFoundError, json.JSONDecodeError):
            saved = {}
        if saved.get("first_run_done") is True and saved.get("send_reports") is False:
            time.sleep(.5)
            subprocess.run(["import", "-window", window, str(output / "onboarding.png")],
                           check=True, timeout=15)
            if app.poll() is not None:
                raise RuntimeError("Launcher exited during onboarding capture")
            # Never export the settings object: it also contains a device identifier.
            return {"settings_saved_by_ui": True, "first_run_done": True,
                    "send_reports": False}
        time.sleep(.1)
    raise RuntimeError("Welcome UI did not persist first_run_done=true, send_reports=false")


def main():
    executable, output = sys.argv[1:]
    output = Path(output)
    manager = subprocess.Popen(["openbox"], stdout=subprocess.DEVNULL,
                               stderr=subprocess.DEVNULL)
    app = None
    try:
        with (output / "app.log").open("w") as log:
            app = subprocess.Popen([executable], stdout=log, stderr=log,
                                   start_new_session=True)
            deadline = time.monotonic() + 40
            window = None
            while time.monotonic() < deadline:
                if app.poll() is not None:
                    raise RuntimeError(f"Launcher exited before window: {app.returncode}")
                found = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid",
                                        str(app.pid), "--name", "DrakeStar"],
                                       capture_output=True, text=True, timeout=5)
                if found.returncode == 0 and found.stdout.strip():
                    window = found.stdout.splitlines()[0]
                    break
                time.sleep(.25)
            if window is None:
                raise RuntimeError("No visible DrakeStar window after 40 seconds")
            time.sleep(6)
            if app.poll() is not None:
                raise RuntimeError(f"Launcher exited after window: {app.returncode}")
            subprocess.run(["import", "-window", window, str(output / "window.png")],
                           check=True, timeout=15)
            title = subprocess.check_output(["xdotool", "getwindowname", window],
                                            text=True, timeout=5).strip()
            if app.poll() is not None:
                raise RuntimeError(f"Launcher exited during capture: {app.returncode}")
            onboarding = None
            if os.environ.get("DRAKESTAR_GUI_ONBOARDING") == "1":
                onboarding = complete_onboarding(
                    app, window, Path(os.environ["DRAKESTAR_DATA_DIR"]), output)
            (output / "gui.json").write_text(json.dumps({
                "uid": os.getuid(), "title": title, "visible_window": True,
                "alive_after_capture": app.poll() is None,
                "render_requires_visual_review": True,
                "onboarding": onboarding,
            }, indent=2) + "\n")
    finally:
        if app is not None:
            try:
                os.killpg(app.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(app.pid, signal.SIGKILL)
                app.wait(timeout=5)
        manager.terminate()
        manager.wait(timeout=5)


if __name__ == "__main__":
    main()
