FROM node:22-bookworm-slim AS web
WORKDIR /src
RUN corepack enable && corepack prepare pnpm@10.29.2 --activate
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY packages ./packages
COPY apps/web ./apps/web
RUN pnpm install --frozen-lockfile && pnpm build

FROM rust:1.98.1-slim-bookworm AS rust
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config build-essential && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY apps/api ./apps/api
COPY apps/worker ./apps/worker
COPY apps/mcp ./apps/mcp
COPY migrations ./migrations
RUN cargo build --release --locked -j 2 -p clickman-api -p clickman-worker -p clickman-mcp-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust /src/target/release/clickman-api /src/target/release/clickman-worker /src/target/release/clickman-mcp-server /usr/local/bin/
COPY --from=web /src/apps/web/dist ./web
ENV CLICKMAN_BIND=0.0.0.0:4132 CLICKMAN_WEB_DIR=/app/web
USER 10001:10001
EXPOSE 4132
CMD ["clickman-api"]
