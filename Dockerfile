FROM rust:1.66.1

COPY ./ ./

RUN cargo build --release

CMD ["./target/release/mahantongo"]
