FROM rust:latest AS chef 

WORKDIR /app
RUN cargo install cargo-chef 

FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

ARG CARGO_PROFILE=release

COPY --from=planner /app/recipe.json recipe.json
# Use cargo-chef to compile dependencies only - this will be cached by Docker.
RUN cargo chef cook --profile $CARGO_PROFILE --recipe-path recipe.json
# ... and then build the rest of the application.
COPY . .
RUN cargo build --profile $CARGO_PROFILE --bin graphix-cross-checker

# Instead of calculating where the binary is located based on $CARGO_PROFILE, we
# simply try to copy both `debug` and `release` binaries.
RUN cp target/release/graphix-cross-checker /usr/local/bin && \
	cp target/debug/graphix-cross-checker /usr/local/bin

FROM debian:bullseye-slim

WORKDIR /app

RUN apt-get update && \
	apt-get install -y libpq-dev

COPY --from=builder /usr/local/bin/graphix-cross-checker /usr/local/bin
COPY --from=builder /app/examples/testing.yml /app/config.yml

EXPOSE 14265

ENTRYPOINT [ "graphix-cross-checker" ]
CMD ["graphix-cross-checker", "--config", "/app/config.yml"]
