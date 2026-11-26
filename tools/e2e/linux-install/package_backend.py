"""Native package operations for the disposable installer fixtures only."""
import hashlib
from pathlib import Path
import subprocess
import tempfile


def command(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT,
                                   timeout=120).strip()


class Debian:
    name = "dpkg"

    def version(self, package):
        if command("dpkg-deb", "-f", str(package), "Package") != "drake-star":
            raise ValueError("Expected official drake-star package")
        if command("dpkg-deb", "-f", str(package), "Architecture") != "amd64":
            raise ValueError("Fixture requires amd64 packages")
        return command("dpkg-deb", "-f", str(package), "Version")

    def require_upgrade(self, older, newer):
        subprocess.run(["dpkg", "--compare-versions", older, "lt", newer],
                       check=True, timeout=5)

    def query(self):
        return ["dpkg-query", "-W", "-f=${Version}", "drake-star"]

    def install(self, package):
        log = command("dpkg", "--install", str(package))
        if command("dpkg", "--audit"):
            raise ValueError("dpkg audit failed")
        return log

    def payload_digest(self, package):
        with tempfile.TemporaryDirectory(prefix="launcher-package-") as directory:
            command("dpkg-deb", "--extract", str(package), directory)
            return hashlib.sha256((Path(directory) / "usr/bin/drakestar-launcher")
                                  .read_bytes()).hexdigest()


class Fedora:
    name = "rpm"
    version_format = "%{EPOCHNUM}:%{VERSION}-%{RELEASE}"

    def version(self, package):
        metadata = command("rpm", "-qp", "--qf",
                           "%{NAME}\n%{ARCH}\n" + self.version_format, str(package))
        name, arch, version = metadata.splitlines()
        if name != "drake-star" or arch != "x86_64":
            raise ValueError("Expected drake-star x86_64 RPM")
        return version

    def require_upgrade(self, older, newer):
        # Use RPM's own epoch/version/release ordering, not lexical comparison.
        import rpm

        def label(version):
            epoch, rest = version.split(":", 1)
            value, release = rest.rsplit("-", 1)
            return epoch, value, release

        if rpm.labelCompare(label(older), label(newer)) >= 0:
            raise ValueError("RPM packages must be in strictly increasing order")

    def query(self):
        return ["rpm", "-q", "--qf", self.version_format, "drake-star"]

    def install(self, package):
        # Keep dependency and file-integrity checks enabled; no --nodeps/--force.
        log = command("rpm", "--upgrade", str(package))
        command("rpm", "--verify", "drake-star")
        return log

    def payload_digest(self, package):
        # Extract only the executable to stdout; never unpack arbitrary RPM paths.
        with tempfile.TemporaryFile() as archive, tempfile.TemporaryFile() as binary:
            subprocess.run(["rpm2cpio", str(package)], stdout=archive,
                           check=True, timeout=30)
            archive.seek(0)
            subprocess.run(["cpio", "--extract", "--to-stdout",
                            "./usr/bin/drakestar-launcher"], stdin=archive,
                           stdout=binary, stderr=subprocess.PIPE, check=True, timeout=30)
            binary.seek(0)
            payload = binary.read()
            if not payload:
                raise ValueError("RPM executable missing from payload")
            return hashlib.sha256(payload).hexdigest()


def select_backend(older, newer):
    if older.suffix == newer.suffix == ".deb":
        return Debian()
    if older.suffix == newer.suffix == ".rpm":
        return Fedora()
    raise ValueError("Expected two .deb packages or two .rpm packages")
