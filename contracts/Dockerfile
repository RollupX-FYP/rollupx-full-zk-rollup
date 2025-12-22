# Use Node.js LTS (Long Term Support) as the base image
FROM node:20-slim

# Set the working directory inside the container
WORKDIR /app

# Copy package.json and package-lock.json (if available)
COPY package.json package-lock.json* ./

# Install dependencies
# We use --legacy-peer-deps to avoid potential conflicts with hardhat plugins/ethers versions if strict
RUN npm ci --legacy-peer-deps || npm install --legacy-peer-deps

# Copy the rest of the application code
COPY . .

# Compile the contracts
RUN npx hardhat compile

# Run tests by default when the container starts
CMD ["npx", "hardhat", "test"]
