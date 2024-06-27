FROM rust:1.78.0 as builder
WORKDIR /app
RUN apt-get update && apt-get install -y musl-tools && rustup target add x86_64-unknown-linux-musl
COPY . .
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM scratch
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/mailhook /mailhook
ENTRYPOINT ["/mailhook"]
EXPOSE 8088 25