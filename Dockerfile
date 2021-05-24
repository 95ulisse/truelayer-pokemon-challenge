FROM rust:1.52 AS builder
WORKDIR /app

# To cache the build dependencies (which take a loooooong time to compile),
# we setup a new dummy project and copy over only the Cargo files
RUN cargo init --bin
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

# Copy the actual source of the project.
# Be sure to `touch` the entrypoint so that cargo thinks they are dirty.
COPY . .
RUN touch src/main.rs && \
    cargo build --release



# Copy the binary to a fresh image
FROM gcr.io/distroless/cc
COPY --from=builder /app/target/release/truelayer-pokemon-challenge /truelayer-pokemon-challenge
USER 1000
ENV PORT 8080
ENTRYPOINT ["/truelayer-pokemon-challenge"]
