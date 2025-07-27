# Build stage
FROM rust:latest as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

# Final stage: minimal image with just the binary
FROM ubuntu:latest
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/agreeboard .
CMD ["./agreeboard"]
