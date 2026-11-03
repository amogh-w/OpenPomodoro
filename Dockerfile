# syntax=docker/dockerfile:1
#
# OpenPomodoro: one small image holding a single program with the web app built into it.
# NOTE: written alongside the project but not yet built in CI; see docs/self-hosting.md.

# 1. Build the web app.
FROM node:22-alpine AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# 2. Build the server, with the web app embedded.
FROM rust:1-bookworm AS server
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY --from=web /src/web/dist ./web/dist
RUN cargo build --release -p op-server --locked

# 3. The runtime image: the program and a data folder, nothing else.
FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin op \
 && mkdir /data && chown op /data
COPY --from=server /src/target/release/op-server /usr/local/bin/op-server
USER op
# Listen on every interface inside the container. The server then insists on OP_PUBLIC_URL (the address people
# type); set it when you run the container.
ENV OP_BIND=0.0.0.0 \
    OP_PORT=3000 \
    OP_DB=/data/openpomodoro.db
VOLUME /data
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD ["op-server", "healthcheck"]
ENTRYPOINT ["op-server"]
