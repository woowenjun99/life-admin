FROM rust:1.95-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --system --uid 10001 app

WORKDIR /app

COPY --from=builder /app/target/release/backend /usr/local/bin/backend

ENV BIND_ADDR=0.0.0.0:3001

EXPOSE 3001

USER app

ENTRYPOINT ["/usr/local/bin/backend"]
