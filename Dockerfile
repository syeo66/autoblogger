FROM rust as builder

WORKDIR /usr/src/app

COPY . .

RUN cargo build -r 

FROM alpine

COPY --from=builder /usr/src/app/target/release/autoblogger /usr/local/bin/autoblogger

CMD ["/usr/local/bin/autoblogger"]
