FROM public.ecr.aws/docker/library/rust:1.90-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    nasm \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY sequencer ./sequencer
COPY proto ./proto

WORKDIR /app/sequencer
RUN cargo build --release --bin sequencer

FROM public.ecr.aws/docker/library/debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    openssl \
    procps \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/sequencer/target/release/sequencer /usr/local/bin/sequencer
COPY sequencer/config /app/config
COPY sequencer/docker-entrypoint.sh /usr/local/bin/sequencer-entrypoint

RUN chmod +x /usr/local/bin/sequencer-entrypoint

EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/sequencer-entrypoint"]
