FROM rust:1.91.1-bookworm as builder
WORKDIR /app

# Install dependencies for compilation
RUN apt-get update && apt-get install -y protobuf-compiler cmake libclang-dev clang

COPY . .

# Build submitter
WORKDIR /app/submitter
RUN cargo build --release --bin submitter

FROM public.ecr.aws/docker/library/debian:bookworm-slim
WORKDIR /app

# Install python3 and yaml for the wrapper
RUN apt-get update && apt-get install -y ca-certificates openssl gettext-base && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/submitter/target/release/submitter /usr/local/bin/submitter
COPY submitter/submitter.docker.yaml /app/submitter.yaml

CMD ["submitter", "--config", "/app/submitter.yaml"]
