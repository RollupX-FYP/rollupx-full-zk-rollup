FROM public.ecr.aws/docker/library/python:3.9-slim

WORKDIR /app

COPY workload/poisson_generator.py .

ENTRYPOINT ["python", "-u", "poisson_generator.py"]
