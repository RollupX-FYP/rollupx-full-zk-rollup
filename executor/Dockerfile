FROM public.ecr.aws/docker/library/rust:1.79 as builder
WORKDIR /app

# Install dependencies for compilation
RUN apt-get update && apt-get install -y protobuf-compiler cmake libclang-dev clang

COPY . .

# Build executor
WORKDIR /app/executor
RUN cargo build --release --bin zksync_state_machine

FROM debian:bookworm-slim
WORKDIR /app

# Install python3 and yaml for the wrapper
RUN apt-get update && apt-get install -y ca-certificates openssl gettext-base python3 python3-yaml && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/executor/target/release/zksync_state_machine /usr/local/bin/executor
COPY executor/run_executor.py /usr/local/bin/run_executor.py
RUN chmod +x /usr/local/bin/run_executor.py

COPY executor/executor.docker.yaml /app/executor.yaml

ENV EXECUTOR_CONFIG=/app/executor.yaml

CMD ["run_executor.py", "executor"]
