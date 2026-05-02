FROM public.ecr.aws/docker/library/python:3.11-slim

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    bc \
    ca-certificates \
    curl \
    git \
    jq \
    gettext-base \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN pip install --no-cache-dir --upgrade pip \
    && pip install --no-cache-dir eth-account

CMD ["python3", "-u", "workload/poisson_generator.py"]
