# =========================
# Stage 1: Build
# =========================
FROM rust:1.90 as builder

# Create app directory
WORKDIR /usr/src/app

# Copy Cargo files first to leverage Docker cache
COPY Cargo.toml Cargo.lock ./

# Build dependencies only
RUN cargo build --release || true

# Copy the full source code
COPY . .

# Build the actual project
RUN cargo build --release

# =========================
# Stage 2: Runtime
# =========================
FROM debian:bookworm-slim

# Install required libraries for Rust binaries (if any)
RUN apt-get update && apt-get install -y \
    libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder
COPY --from=builder /usr/src/app/target/release/icm-server /usr/local/bin/icm-server

# Set working directory
WORKDIR /usr/local/bin

# Expose your server port (adjust as needed)
EXPOSE 3000

# Run the binary
CMD ["./icm-server"]
