FROM rust:latest AS chef 

WORKDIR /app
COPY rust-toolchain.toml .
# Triggers the install of the predefined Rust toolchain.
# See <https://github.com/rust-lang/rustup/issues/1397>.
RUN rustup show
RUN cargo install cargo-chef 

FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

# We support both `release` and `dev`.
ARG CARGO_PROFILE=release

COPY --from=planner /app/recipe.json recipe.json
# Use cargo-chef to compile dependencies only - this will be cached by Docker.
RUN cargo chef cook --profile $CARGO_PROFILE --recipe-path recipe.json --bin graphix-api-server
# ... and then build the rest of the application.
COPY . .
RUN cargo build --profile $CARGO_PROFILE --bin graphix-api-server

# Instead of calculating where the binary is located based on $CARGO_PROFILE, we
# simply try to copy both `debug` and `release` binaries.
RUN cp target/release/graphix-api-server /usr/local/bin | true && \
	cp target/debug/graphix-api-server /usr/local/bin | true

FROM debian:bullseye-slim

WORKDIR /app

RUN apt-get update && \
	apt-get install -y libpq-dev

COPY --from=builder /usr/local/bin/graphix-api-server /usr/local/bin

EXPOSE 3030

ENTRYPOINT [ "graphix-api-server" ]
CMD ["graphix-api-server", "--port", "3030"]
