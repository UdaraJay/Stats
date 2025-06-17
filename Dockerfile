FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin stats

FROM rust AS diesel-builder

RUN apt update && \
    apt install -y libsqlite3-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
RUN cargo install diesel_cli --no-default-features --features sqlite --root /app

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
WORKDIR /app

ENV APP_URL=http://127.0.0.1:5775
ENV SERVICE_PORT=5775
ENV DATABASE_URL=/app/data/stats.sqlite
ENV PROCESSING_BATCH_SIZE=500
# comma-seperated
ENV CORS_DOMAINS=http://localhost:5775

RUN apt update && \
    apt install -y libsqlite3-0 && \
    rm -rf /var/lib/apt/lists/*

# Copy necessary files to /data
WORKDIR /app

ADD https://git.io/GeoLite2-City.mmdb /app/data/GeoLite2-City.mmdb
ADD https://github.com/PrismaPhonic/filter-cities-by-country/raw/master/cities5000.txt /app/data/cities5000.txt

COPY migrations/ /app/migrations
COPY ui/ /app/ui
COPY --from=diesel-builder /app/bin/diesel /app
COPY --from=builder /app/target/release/stats /app

ENV PATH="/app:${PATH}"
EXPOSE ${SERVICE_PORT}

CMD diesel migration run && stats
