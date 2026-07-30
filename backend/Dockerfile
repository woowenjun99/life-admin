FROM rust:1.95-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        tini \
        util-linux \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --system --uid 10001 app

WORKDIR /app

COPY --from=builder /app/target/release/backend /usr/local/bin/backend
COPY docker/start-backend /usr/local/bin/start-backend

RUN chmod 0755 /usr/local/bin/start-backend

ENV BIND_ADDR=0.0.0.0:3001

EXPOSE 3001

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/start-backend"]
