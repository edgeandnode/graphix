FROM rust:slim-bullseye AS builder 

WORKDIR /app

RUN	apt-get update && apt-get install -y libpq-dev ca-certificates pkg-config libssl-dev

COPY . .

RUN cargo build --release --bin graphix
RUN cp target/release/graphix /usr/local/bin

FROM debian:bullseye-slim

WORKDIR /app

RUN apt-get update && \
	apt-get install -y libpq-dev ca-certificates libssl-dev && \
	apt-get clean

COPY --from=builder /usr/local/bin/graphix /usr/local/bin

ENV RUST_LOG="graphix=debug"

EXPOSE 3030

ENTRYPOINT [ "graphix" ]
