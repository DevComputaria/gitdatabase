FROM rust:1.80 AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations

RUN cargo build --release -p gitbase-cli

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/gitbase /usr/local/bin/gitbase

EXPOSE 5433

ENTRYPOINT ["/usr/local/bin/gitbase"]
