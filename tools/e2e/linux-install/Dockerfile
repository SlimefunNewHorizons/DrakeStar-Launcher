FROM ubuntu:24.04@sha256:2198e4c65076e9cb0b3d695219df667c0e3b0072da64ca3c2648aca17dcecaf5
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    libwebkit2gtk-4.1-0 libgtk-3-0t64 python3 xvfb xauth xdotool imagemagick \
    dbus-x11 openbox fonts-dejavu-core ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 smoke
COPY install-smoke.py gui-probe.py package_backend.py /fixture/
ENV DRAKESTAR_INSTALL_FIXTURE=1
ENTRYPOINT ["python3", "/fixture/install-smoke.py"]
