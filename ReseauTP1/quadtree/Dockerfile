FROM rust:latest

# Installation des dépendances graphiques indispensables pour Bevy (X11, Wayland, Alsa pour le son, etc.)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libx11-dev \
    libasound2-dev \
    libudev-dev \
    libwayland-dev \
    libxkbcommon-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/myapp