# Build Stage
FROM rust:1.85-slim as builder
WORKDIR /usr/src/raw-redis
COPY . .
RUN cargo build --release

# Run Stage
FROM debian:bookworm-slim
WORKDIR /usr/local/bin
# Copy the binary from the builder stage
COPY --from=builder /usr/src/raw-redis/target/release/mini-redis .
# Expose the port your app binds to
EXPOSE 6379
CMD ["./mini-redis"]