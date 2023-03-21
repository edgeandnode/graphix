FROM rust:latest as builder

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:bullseye-slim

WORKDIR /app

RUN apt-get update && \
	apt-get install -y libpq-dev

COPY --from=builder /app/target/release/graph-ixi-cross-checker /usr/local/bin
COPY --from=builder /app/examples/testing.yml /app/config.yml

EXPOSE 14265

CMD ["graph-ixi-cross-checker", "--config", "/app/config.yml"]
