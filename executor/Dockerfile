FROM public.ecr.aws/docker/library/rust:1.90-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    clang \
    cmake \
    curl \
    git \
    libssl-dev \
    nasm \
    pkg-config \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

RUN rustup toolchain install nightly-2024-08-01 && \
    rustup component add rustfmt --toolchain nightly-2024-08-01

RUN curl -L https://risczero.com/install | bash
ENV PATH="/root/.risc0/bin:${PATH}"
RUN rzup install

COPY executor ./executor
COPY proto ./proto
COPY risc0_prover ./risc0_prover

ENV CXXFLAGS="-include cstdint"

RUN cargo +nightly-2024-08-01 build --release \
    --manifest-path executor/Cargo.toml \
    -p zksync_state_machine \
    --ignore-rust-version

# Build the RISC Zero host through the workspace so risc0-build compiles and
# embeds a real guest ELF. Do not set RISC0_SKIP_BUILD here: an empty embedded
# method image causes the runtime "Elf parse error" seen during proof generation.
RUN cargo build --release \
    --manifest-path risc0_prover/Cargo.toml \
    -p rollup_host

FROM public.ecr.aws/docker/library/debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    openssl \
    procps \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /var/lib/rollupx/executor/traces \
    /var/lib/rollupx/executor/state_db \
    /var/lib/rollupx/executor/risc0

COPY --from=builder /app/executor/target/release/zksync_state_machine /usr/local/bin/zksync_state_machine
COPY --from=builder /app/risc0_prover/target/release/rollup_host /usr/local/bin/rollup_host
COPY --from=builder /root/.risc0 /root/.risc0
COPY executor/docker-entrypoint.sh /usr/local/bin/executor-entrypoint

RUN chmod +x /usr/local/bin/executor-entrypoint

ENV PROVER_BACKEND=risc0
ENV RISC0_HOST_BIN=/usr/local/bin/rollup_host
ENV PATH="/root/.risc0/bin:${PATH}"
ENV EXECUTOR_GRPC_ADDR=0.0.0.0:50051
ENV TRACE_ROOT=/var/lib/rollupx/executor/traces
ENV STATE_DB_PATH=/var/lib/rollupx/executor/state_db
ENV RISC0_WORK_DIR=/var/lib/rollupx/executor/risc0

EXPOSE 50051

ENTRYPOINT ["/usr/local/bin/executor-entrypoint"]
