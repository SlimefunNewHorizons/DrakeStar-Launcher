#!/usr/bin/env python3
"""Run as the fixture's unprivileged user inside dbus-run-session + Xvfb."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


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
            (output / "gui.json").write_text(json.dumps({
                "uid": os.getuid(), "title": title, "visible_window": True,
                "alive_after_capture": app.poll() is None,
                "render_requires_visual_review": True,
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
