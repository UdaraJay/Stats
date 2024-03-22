# Use the official Rust image as the base image
FROM rust:latest

# Install PostgreSQL client (required for Diesel CLI with PostgreSQL)
RUN apt-get update && apt-get install -y libpq-dev postgresql-client && rm -rf /var/lib/apt/lists/*

# Install Diesel CLI
RUN cargo install diesel_cli --no-default-features --features sqlite

# Set the working directory
WORKDIR /app

# Copy the necessary files over
COPY migrations /app/migrations
COPY ui /app/ui

COPY data/cities5000.txt /app/data/cities5000.txt
COPY data/GeoLite2-City.mmdb /app/data/GeoLite2-City.mmdb
COPY data/stats.sqlite /app/data/stats.sqlite

COPY .env-docker /app/.env

COPY Cargo.toml /app/Cargo.toml
COPY diesel.toml /app/diesel.toml
COPY src /app/src

# Build the application
RUN cargo build --release

# Run the diesel migration
RUN diesel migration run

# Expose the port
EXPOSE 5775

# Run the application
CMD ["/app/target/release/stats"]