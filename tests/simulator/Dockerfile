FROM rust:1.91.1-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    libsdl2-dev pkg-config python3 python3-tk python3-pil python3-venv xvfb xauth xdotool \
    openbox imagemagick x11-utils curl ca-certificates openssl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
ENV CARGO_TARGET_DIR=/target DISPLAY=:99
CMD ["sh", "tests/simulator/run.sh"]
