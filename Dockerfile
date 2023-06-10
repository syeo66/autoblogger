FROM rust as builder

WORKDIR /usr/src/app

COPY . .

RUN cargo build -r 

FROM debian:bullseye-slim

COPY --from=builder /usr/src/app/target/release/autoblogger /usr/local/bin/autoblogger

CMD ["autoblogger"]
