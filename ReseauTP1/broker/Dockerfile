FROM rust:latest

# Installation des dépendances système nécessaires pour compiler GameNetworkingSockets et Protobuf
RUN apt-get update && apt-get install -y \
    clang \
    cmake \
    build-essential \
    protobuf-compiler \
    libprotobuf-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Définition de la variable d'environnement pour que bindgen trouve libclang à coup sûr
ENV LIBCLANG_PATH=/usr/lib/llvm-19/lib

WORKDIR /usr/src/myapp