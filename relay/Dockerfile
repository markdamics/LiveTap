# syntax=docker/dockerfile:1

FROM rust:1-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /build

# Build in the context of the whole workspace so the `livetap-shared` path
# dependency resolves.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY relay ./relay

# The official rust:alpine image already targets musl by default, so this
# produces a static-ish binary without needing an explicit --target/rustup
# target add step.
RUN cargo build --release --package livetap-relay

FROM alpine:3
RUN adduser -D -u 10001 livetap
COPY --from=builder /build/target/release/livetap-relay /usr/local/bin/livetap-relay
USER livetap
ENV LIVETAP_RELAY_ADDR=0.0.0.0:8787
EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/livetap-relay"]
