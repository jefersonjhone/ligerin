# Sintese: client Tauri com UI embutida, rodando headless com Xvfb + VNC.
# Build em 3 estágios: frontend (bun/vite) -> rust com deps do Tauri ->
# runtime debian com WebKitGTK, Xvfb e x11vnc.
#
# Uso:
#   docker build -f client-motorista/Dockerfile \
#     --build-arg CLIENT=client-motorista --build-arg BIN=ligerin-client-motorista -t ligerin-motorista .
ARG CLIENT=client-motorista
ARG BIN=ligerin-client-motorista

# ── estágio 1: frontend buildado (vite -> dist/) ─────────────────────────────
FROM oven/bun:1 AS frontend
ARG CLIENT
WORKDIR /src
COPY ${CLIENT}/package.json ${CLIENT}/bun.lock ./
COPY ${CLIENT}/index.html ${CLIENT}/vite.config.ts ${CLIENT}/tsconfig.json ${CLIENT}/tsconfig.node.json ./
COPY ${CLIENT}/src ./src
RUN bun install --frozen-lockfile && bun run build

# ── estágio 2: compilação do Tauri com a UI embutida ─────────────────────────
FROM rust:1-slim-bookworm AS build
ARG CLIENT
ARG BIN
RUN apt-get update && apt-get install -y --no-install-recommends \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    patchelf \
    build-essential \
    pkg-config \
    curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# src-tauri depende de ligerin-protocol com path ../../protocol → raiz do FS
COPY protocol/ /protocol/
COPY ${CLIENT}/src-tauri/ ./src-tauri/
# frontendDist do tauri.conf.json é "../dist" em relação ao src-tauri
COPY --from=frontend /src/dist ./dist
WORKDIR /app/src-tauri
# custom-protocol: sem ela o binário usa o devUrl (localhost:1420) em vez do bundle
RUN cargo build --release --features tauri/custom-protocol

# ── estágio 3: runtime headless (WebKit + Xvfb + x11vnc + app) ───────────────
FROM debian:bookworm-slim
ARG BIN
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    libwebkit2gtk-4.1-0 \
    libgtk-3-0 \
    libayatana-appindicator3-1 \
    librsvg2-2 \
    xvfb \
    x11vnc \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/src-tauri/target/release/${BIN} /app/app
# o server LPC é resolvido pelo nome do serviço no compose (default: local)
ENV LPC_SERVER_HOST=127.0.0.1 \
    LPC_SERVER_PORT=8080 \
    DISPLAY=:99
EXPOSE 5900
# Subida segura: espera o Xvfb criar o socket do display antes do app.
CMD ["sh", "-c", "rm -f /tmp/.X99-lock; Xvfb :99 -screen 0 1280x800x24 & i=0; while [ ! -S /tmp/.X11-unix/X99 ] && [ $i -lt 20 ]; do sleep 1; i=$((i+1)); done; x11vnc -display :99 -forever -nopw -rfbport 5900 & exec ./app"]