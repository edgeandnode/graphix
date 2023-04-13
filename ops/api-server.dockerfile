FROM rust:latest as builder

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:bullseye-slim

RUN apt-get update && \
	apt-get install -y libssl1.1 libpq-dev

COPY --from=builder /app/target/release/graph-ixi-api-server /usr/local/bin

EXPOSE 3030

ENTRYPOINT [ "graph-ixi-api-server" ]
CMD ["graph-ixi-api-server", "--port", "3030"]
