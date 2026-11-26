#!/usr/bin/env python3
"""Comprueba el binario GUI Linux o un .deb sin ejecutar ni instalar su contenido."""

import argparse
import configparser
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import tempfile


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def check_binary(binary):
    dynamic = run("readelf", "--dynamic", str(binary))
    needed = re.findall(r"\(NEEDED\).*\[([^\]]+)\]", dynamic)
    for library in ("libwebkit2gtk-", "libgtk-"):
        if not any(name.startswith(library) for name in needed):
            raise ValueError(f"El binario no enlaza {library}: posible CLI empaquetado como GUI")
    return {
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "needed": needed,
    }


def check_deb(artifact, version):
    actual_version = run("dpkg-deb", "--field", str(artifact), "Version").strip()
    if actual_version != version:
        raise ValueError(f"Version del paquete {actual_version!r}; esperada {version!r}")
    with tempfile.TemporaryDirectory(prefix="drakestar-release-") as directory:
        root = Path(directory)
        run("dpkg-deb", "--extract", str(artifact), str(root))
        desktops = list((root / "usr/share/applications").glob("*.desktop"))
        if len(desktops) != 1:
            raise ValueError("Se esperaba una entrada de escritorio")
        desktop = configparser.ConfigParser(interpolation=None)
        desktop.read(desktops[0])
        entry = desktop["Desktop Entry"]
        command = shlex.split(entry["Exec"])[0]
        if Path(command).name != command or entry.getboolean("Terminal", fallback=True):
            raise ValueError("Entrada de escritorio inesperada: Exec simple y Terminal=false requeridos")
        result = check_binary(root / "usr/bin" / command)
        result.update(version=actual_version, desktop_exec=command)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path, help="Binario ELF o paquete .deb")
    parser.add_argument("--version", help="Version exacta requerida para un .deb")
    args = parser.parse_args()
    if args.artifact.suffix == ".deb" and not args.version:
        parser.error("--version es obligatorio para un .deb")
    try:
        result = (check_deb(args.artifact, args.version) if args.artifact.suffix == ".deb"
                  else check_binary(args.artifact))
        result["artifact_sha256"] = hashlib.sha256(args.artifact.read_bytes()).hexdigest()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"ERROR: {error}\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
