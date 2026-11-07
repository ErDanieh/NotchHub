# Dev and test environment for the Linux port (Ubuntu 24.04, same as a current Ubuntu desktop).
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config curl ca-certificates git \
    libgtk-4-dev libglib2.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev libpango1.0-dev \
    libpulse-dev libpoppler-glib-dev libx11-dev \
    xvfb openbox dbus dbus-x11 x11-utils xdotool imagemagick ffmpeg playerctl \
    python3 python3-gi gir1.2-glib-2.0 fonts-dejavu-core fonts-noto-color-emoji \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH=/root/.cargo/bin:$PATH
RUN cargo install cargo-deb --locked
WORKDIR /work/linux
RUN apt-get update && apt-get install -y --no-install-recommends xcompmgr x11-xserver-utils && rm -rf /var/lib/apt/lists/*
RUN apt-get update && apt-get install -y --no-install-recommends picom && rm -rf /var/lib/apt/lists/*
RUN apt-get update && apt-get install -y --no-install-recommends adwaita-icon-theme gnome-themes-extra && rm -rf /var/lib/apt/lists/*
RUN apt-get update && apt-get install -y --no-install-recommends librsvg2-common && rm -rf /var/lib/apt/lists/*
