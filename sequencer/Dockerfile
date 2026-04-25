FROM rust:1.91.1-bookworm as builder
WORKDIR /app

# Install dependencies for compilation
RUN apt-get update && apt-get install -y protobuf-compiler

COPY . .

# Build sequencer
WORKDIR /app/sequencer
RUN cargo build --release --bin sequencer

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates openssl gettext-base && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/sequencer/target/release/sequencer /usr/local/bin/sequencer
COPY sequencer/sequencer.docker.yaml /app/sequencer.yaml

ENV SEQUENCER_CONFIG=/app/sequencer.yaml

CMD ["sequencer"]
