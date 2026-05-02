FROM public.ecr.aws/docker/library/python:3.11-slim

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    && rm -rf /var/lib/apt/lists/*

COPY data-tools ./data-tools
COPY run_pipeline.sh ./run_pipeline.sh

RUN pip install --no-cache-dir --upgrade pip \
    && pip install --no-cache-dir pandas matplotlib numpy

ENV METRICS_ROOT=/var/lib/rollupx/metrics

CMD ["bash", "-lc", "./run_pipeline.sh"]
