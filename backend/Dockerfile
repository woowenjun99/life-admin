FROM debian:bookworm-slim AS scanner-base

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        clamav-daemon \
        clamav-freshclam \
        netcat-openbsd \
        tini \
        util-linux \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir --parents /var/lib/clamav \
    && chown --recursive clamav:clamav /var/lib/clamav

COPY docker/clamd.conf /etc/clamav/clamd.conf
COPY docker/freshclam.conf /etc/clamav/freshclam.conf
COPY docker/start-clamav /usr/local/bin/start-clamav
COPY docker/start-backend /usr/local/bin/start-backend

RUN chmod 0755 /usr/local/bin/start-clamav /usr/local/bin/start-backend

FROM rust:1.95-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src

RUN cargo build --release --locked

FROM scanner-base AS scanner

# Docker publishes ports to the container network interface rather than its
# loopback interface. The host mapping in compose.yaml remains loopback-only.
COPY docker/clamd-compose.conf /etc/clamav/clamd.conf

EXPOSE 3310

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/start-clamav"]

FROM scanner-base AS runtime

RUN useradd --create-home --system --uid 10001 app

WORKDIR /app

COPY --from=builder /app/target/release/backend /usr/local/bin/backend

ENV BIND_ADDR=0.0.0.0:3001

EXPOSE 3001

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/start-backend"]
