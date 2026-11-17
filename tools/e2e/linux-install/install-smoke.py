#!/usr/bin/env python3
"""Install/upgrade official .debs ONLY inside the disposable Docker fixture."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile


def command(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT,
                                   timeout=120).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("older", type=Path)
    parser.add_argument("newer", type=Path)
    args = parser.parse_args()
    if (not Path("/.dockerenv").is_file() or os.getuid() != 0
            or os.environ.get("DRAKESTAR_INSTALL_FIXTURE") != "1"):
        parser.error("Requires the disposable linux-install Docker image; never run on host")
    # Checking IPv4 routes alone misses interfaces with IPv6 or no route yet.
    if {entry.name for entry in Path("/sys/class/net").iterdir()} != {"lo"}:
        parser.error("Requires docker run --network none")
    result = {"ok": False, "steps": [], "scope": "dpkg upgrade; not in-app updater",
              "network_interfaces": ["lo"],
              "os_release": Path("/etc/os-release").read_text()}
    output = Path("/evidence")
    output.mkdir(exist_ok=True)
    try:
        versions = []
        for package in (args.older, args.newer):
            if command("dpkg-deb", "-f", str(package), "Package") != "drake-star":
                raise ValueError("Expected official drake-star package")
            if command("dpkg-deb", "-f", str(package), "Architecture") != "amd64":
                raise ValueError("Fixture requires amd64 packages")
            versions.append(command("dpkg-deb", "-f", str(package), "Version"))
        subprocess.run(["dpkg", "--compare-versions", versions[0], "lt", versions[1]],
                       check=True, timeout=5)
        installed = subprocess.run(["dpkg-query", "-W", "drake-star"],
                                   capture_output=True, timeout=5)
        if installed.returncode == 0:
            raise ValueError("Launcher already installed: use a fresh container")
        data = Path("/home/smoke/.drakestar")
        if data.exists():
            raise ValueError("Fixture already has launcher data: use a fresh container")
        marker = data / "instances/installer-smoke/saves/sentinel.txt"
        marker.parent.mkdir(parents=True)
        marker.write_text("Synthetic save: preserve through package upgrade\n")
        subprocess.run(["chown", "-R", "10001:10001", str(data)], check=True, timeout=5)
        expected_marker = digest(marker)
        for index, (package, version) in enumerate(zip((args.older, args.newer), versions)):
            step = {"version": version, "artifact_sha256": digest(package)}
            result["steps"].append(step)
            phase = output / ("install" if index == 0 else "upgrade")
            phase.mkdir()
            (phase / "dpkg.log").write_text(command("dpkg", "--install", str(package)))
            actual = command("dpkg-query", "-W", "-f=${Version}", "drake-star")
            if actual != version:
                raise ValueError(f"Installed version {actual}, expected {version}")
            if command("dpkg", "--audit"):
                raise ValueError("dpkg audit failed")
            desktops = list(Path("/usr/share/applications").glob("*Drake*.desktop"))
            if len(desktops) != 1:
                raise ValueError(f"Expected one launcher desktop entry, got {desktops}")
            fields = dict(line.split("=", 1) for line in desktops[0].read_text().splitlines()
                          if "=" in line)
            executable = shlex.split(fields["Exec"])
            if executable != ["drakestar-launcher"] or fields.get("Terminal") != "false":
                raise ValueError("Unexpected desktop entry")
            with tempfile.TemporaryDirectory(prefix="launcher-package-") as directory:
                command("dpkg-deb", "--extract", str(package), directory)
                packaged = Path(directory) / "usr/bin/drakestar-launcher"
                expected_binary = digest(packaged)
            if digest(Path("/usr/bin/drakestar-launcher")) != expected_binary:
                raise ValueError("Installed executable differs from package payload")
            step["installed_binary_sha256"] = expected_binary
            subprocess.run(["chown", "10001:10001", str(phase)], check=True, timeout=5)
            with (phase / "session.log").open("w") as log:
                subprocess.run([
                    "runuser", "-u", "smoke", "--", "env",
                    "DRAKESTAR_DATA_DIR=" + str(data), "LIBGL_ALWAYS_SOFTWARE=1",
                    "DRAKESTAR_GUI_ONBOARDING=" + ("1" if version == "0.6.0" else "0"),
                    "GDK_BACKEND=x11", "WEBKIT_DISABLE_DMABUF_RENDERER=1",
                    # WebKit's nested namespace sandbox cannot run under Docker's
                    # default seccomp. Only this offline, disposable fixture opts out.
                    "WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1",
                    "dbus-run-session", "--", "xvfb-run", "-a", "-s",
                    "-screen 0 1280x900x24 -nolisten tcp", "python3",
                    "/fixture/gui-probe.py", executable[0], str(phase),
                ], check=True, timeout=90, stdout=log, stderr=log)
            if digest(marker) != expected_marker:
                raise ValueError("Synthetic save changed during install/upgrade")
            step.update(installed_version=actual, marker_preserved=True,
                        marker_sha256=expected_marker,
                        gui=json.loads((phase / "gui.json").read_text()))
        result["ok"] = True
    except Exception as error:
        result["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
