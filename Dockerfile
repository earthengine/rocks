FROM rust:1.59

ENV RUST_LOG="rocks=debug"

WORKDIR /usr/src/rocks
COPY . .

RUN cargo install --path .

EXPOSE 8443

CMD ["rocks"]