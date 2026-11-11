# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown \
    && curl -fsSL https://github.com/leptos-rs/cargo-leptos/releases/download/v0.3.11/cargo-leptos-x86_64-unknown-linux-gnu.tar.gz \
       | tar -xz --strip-components=1 -C /usr/local/cargo/bin

WORKDIR /src
COPY . .

# Cache mounts keep downloaded crates and compiled dependencies between builds.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo leptos build --release \
    && mkdir -p /out \
    && cp target/release/aeon-server-explorer /out/aeon-server-explorer \
    && cp -r target/site /out/site


# The docker CLI and the compose plugin, for Redeploy. Both are static binaries.
FROM docker:28-cli AS dockercli


FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app explorer \
    && mkdir -p /app/data /usr/local/libexec/docker/cli-plugins \
    && chown explorer /app/data

COPY --from=dockercli /usr/local/bin/docker /usr/local/bin/docker
COPY --from=dockercli /usr/local/libexec/docker/cli-plugins/docker-compose /usr/local/libexec/docker/cli-plugins/docker-compose

WORKDIR /app
COPY --from=builder /out/aeon-server-explorer /app/aeon-server-explorer
COPY --from=builder /out/site /app/site

ENV LEPTOS_OUTPUT_NAME=explorer \
    LEPTOS_SITE_ROOT=site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=127.0.0.1:5050 \
    LEPTOS_RELOAD_PORT=5051 \
    LEPTOS_ENV=PROD

USER explorer
CMD ["/app/aeon-server-explorer"]
