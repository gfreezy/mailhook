FROM alpine:latest
COPY target/x86_64-unknown-linux-musl/release/mailhook /mailhook
ENTRYPOINT ["/mailhook"]
