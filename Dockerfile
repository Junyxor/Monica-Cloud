# Monica Cloud: one image with the server binary and the built WebUI,
# following vaultwarden's single-container layout. Data lives in /data.

FROM node:24-bookworm-slim AS webui
WORKDIR /src/webui
COPY webui/package.json webui/package-lock.json ./
RUN npm ci
COPY webui/ ./
RUN npm run build

FROM rust:1-bookworm AS server
WORKDIR /src/server
COPY server/ ./
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /src/server/target/release/monica-cloud /app/monica-cloud
COPY --from=webui /src/webui/dist /app/web-vault
ENV DATA_FOLDER=/data \
    WEB_VAULT_FOLDER=/app/web-vault \
    ADDRESS=0.0.0.0 \
    PORT=80
VOLUME /data
EXPOSE 80
HEALTHCHECK --interval=60s --timeout=10s CMD curl -fsS "http://127.0.0.1:${PORT}/alive" || exit 1
CMD ["/app/monica-cloud"]
