FROM rust:1.36

RUN rustup install nightly
RUN rustup default nightly

ENV RUST_LOG="rocks=debug"

WORKDIR /usr/src/rocks
COPY . .

RUN cargo install --path .

EXPOSE 8443

CMD ["rocks"]