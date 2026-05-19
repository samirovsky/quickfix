# bot-service container image.
#
# Build context is the **crate directory**, not the repo root:
#   docker build -t bot-service rust/bot-service
# That's also what `flyctl deploy` uses with the fly.toml in this dir,
# so local builds and the GH Actions deploy stay in sync.
#
# Run locally:
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
        ca-certificates pkg-config libssl-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies first. Copy the manifest, build a dummy main, fetch,
# then copy the rest. Sources change far more often than Cargo.toml.
COPY Cargo.toml ./
COPY Cargo.lock* ./
RUN mkdir -p src && echo 'fn main(){}' > src/main.rs && cargo fetch

# Real sources. build.rs needs proto/ to exist, so copy that before the
# blanket `.` copy in case the cache is hot for everything else.
COPY proto ./proto
COPY build.rs ./
COPY src ./src
COPY templates ./templates
COPY migrations ./migrations
RUN cargo build --release --bin bot-service

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /work/target/release/bot-service /usr/local/bin/bot-service

ENV BOT_SERVICE_BIND=0.0.0.0:9100 \
    BOT_SERVICE_DB=sqlite:///data/bot.sqlite \
    RUST_LOG=info
VOLUME ["/data"]
EXPOSE 9100

ENTRYPOINT ["bot-service"]
