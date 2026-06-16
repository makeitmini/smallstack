FROM rust:1-slim-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

RUN cargo build -p demo --release

FROM debian:bookworm-slim AS runtime

COPY --from=builder /app/target/release/demo /usr/local/bin/demo
COPY --from=builder /app/crates/demo/public /app/public

WORKDIR /app
EXPOSE 3000
CMD ["demo"]
