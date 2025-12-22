FROM node:20-alpine

WORKDIR /app

# Install deps (including devDeps) using lockfile
COPY package.json package-lock.json ./
RUN npm ci

# Copy source
COPY . .

# Compile (will work now)
RUN npx hardhat compile

# Default: run tests
CMD ["npx", "hardhat", "test"]
