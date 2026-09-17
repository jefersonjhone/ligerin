# Síntese: server LPC (imutável em memória, sem display necessário).
# Multi-stage: compila em rust:1-slim, roda em debian slim com o grafo.

FROM rust:1-slim-bookworm AS build
WORKDIR /app
COPY protocol/ protocol/
COPY server/ server/
WORKDIR /app/server
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=build /app/server/target/release/ligerin /app/ligerin
# grafo rodoviário + malha dos municípios (o server procura em data/ e ../data/)
COPY data/ data/
EXPOSE 8080
CMD ["./ligerin"]