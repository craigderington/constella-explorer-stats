# syntax=docker/dockerfile:1

# ---- build -----------------------------------------------------------------
FROM rust:1-bookworm AS build
WORKDIR /src

# Cargo.toml declares an `examples/` target set; the release binary does not
# need it, so only what the binary compiles is copied.
COPY Cargo.toml Cargo.lock ./
COPY migration migration
COPY src src
COPY assets assets

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --bin loco_sample-cli \
 && cp target/release/loco_sample-cli /loco_sample-cli

# ---- runtime ---------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin app

WORKDIR /app
COPY --from=build /loco_sample-cli /app/loco_sample-cli
COPY config /app/config

USER 10001:10001
ENV LOCO_ENV=production
EXPOSE 5150

HEALTHCHECK --interval=15s --timeout=3s --start-period=20s --retries=3 \
  CMD curl -fsS http://127.0.0.1:5150/_ping || exit 1

ENTRYPOINT ["/app/loco_sample-cli"]
CMD ["start"]
