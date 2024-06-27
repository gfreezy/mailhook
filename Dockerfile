FROM rust:alpine as builder
WORKDIR /app
COPY . .
ENV OPENSSL_NO_VENDOR=Y
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:latest
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/mailhook /mailhook
ENTRYPOINT ["/mailhook"]
EXPOSE 8088 25