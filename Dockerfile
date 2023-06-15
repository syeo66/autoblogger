FROM rust as builder

WORKDIR /usr/src/app

COPY . .

RUN cargo build -r 

FROM debian:bullseye

COPY --from=builder /usr/src/app/target/release/autoblogger /usr/local/bin/autoblogger
RUN apt-get update
RUN apt-get install -y ca-certificates

CMD ["autoblogger"]
