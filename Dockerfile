FROM rust:latest

WORKDIR /usr/src/app

# Copy all files
COPY . .

# Build release binary
RUN cargo build --release

# Run the binary (adjust name if different)
CMD ["./target/release/agreeboard"]
