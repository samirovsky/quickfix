# bot-service container image.
#
# Two-stage build: a debian-slim Rust toolchain compiles a release
# binary, then a thin debian-slim runtime copies just the binary and the
# templates JSON. sqlite is bundled (libsqlite3-sys vendors it via the
# `sqlite` feature on sqlx).
#
# Usage:
#   docker build -t bot-service -f rust/bot-service/Dockerfile .
#   docker run --rm -p 9100:9100 \
#       -v $(pwd)/data:/data \
#       -e BOT_SERVICE_BIND=0.0.0.0:9100 \
#       -e BOT_SERVICE_DB=sqlite:///data/bot.sqlite \
#       -e BOT_SERVICE_SEED_DEMO=1 \
#       -e BOT_SERVICE_CORS_ORIGINS=https://your-app.vercel.app \
#       bot-service

FROM rust:1.83-slim-bookworm AS builder
WORKDIR /work
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies first.
COPY rust/bot-service/Cargo.toml rust/bot-service/Cargo.toml
COPY rust/bot-service/Cargo.lock* rust/bot-service/
RUN mkdir -p rust/bot-service/src && echo 'fn main(){}' > rust/bot-service/src/main.rs
RUN cd rust/bot-service && cargo fetch

# Real sources.
COPY rust/bot-service rust/bot-service
RUN cd rust/bot-service && cargo build --release --bin bot-service

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /work/rust/bot-service/target/release/bot-service /usr/local/bin/bot-service

# Templates are baked in via include_str! at compile time, so we don't
# need to copy templates/library.json into the runtime image.

ENV BOT_SERVICE_BIND=0.0.0.0:9100 \
    BOT_SERVICE_DB=sqlite:///data/bot.sqlite \
    RUST_LOG=info
VOLUME ["/data"]
EXPOSE 9100

ENTRYPOINT ["bot-service"]
