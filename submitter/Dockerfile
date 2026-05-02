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

COPY proto ./proto
COPY submitter ./submitter

WORKDIR /app/submitter
RUN cargo build --release --bin submitter

FROM public.ecr.aws/docker/library/debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    openssl \
    procps \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /app/data /app/offchain_store

COPY --from=builder /app/submitter/target/release/submitter /usr/local/bin/submitter
COPY submitter/submitter.yaml /app/submitter.yaml
COPY submitter/config /app/config

EXPOSE 9000

ENTRYPOINT ["submitter"]
CMD ["--config", "/app/submitter.yaml"]
