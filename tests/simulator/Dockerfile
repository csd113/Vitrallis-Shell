FROM rust:1.91.1-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    libsdl2-dev pkg-config python3 python3-tk python3-pil python3-venv python3-packaging xvfb xauth xdotool \
    openbox awesome dbus-x11 lua5.3 xterm imagemagick x11-utils curl ca-certificates openssl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
ENV CARGO_TARGET_DIR=/target DISPLAY=:99
CMD ["sh", "tests/simulator/run.sh"]
