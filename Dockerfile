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

# Environment configuration
ENV PORT 8080
ENV SHAKESPEARE_TRANSLATOR_ENDPOINT https://api.funtranslations.com/
ENV POKEAPI_ENDPOINT https://pokeapi.co/api/v2/
ENV POKEAPI_CACHE_SIZE 100

USER 1000
ENTRYPOINT ["/truelayer-pokemon-challenge"]
