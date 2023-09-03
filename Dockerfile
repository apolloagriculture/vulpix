FROM rust:alpine as build

RUN apk add libc-dev clang-dev

RUN USER=root cargo new --bin vulpix
WORKDIR /vulpix

RUN apk --update add imagemagick-dev openssl-dev


COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN RUSTFLAGS="-C target-feature=-crt-static" cargo build --release

RUN rm src/*.rs
COPY ./src ./src
COPY ./config ./config

RUN rm ./target/release/deps/vulpix*
RUN RUSTFLAGS="-C target-feature=-crt-static" cargo build --release


FROM alpine
COPY --from=build /vulpix/target/release/vulpix .
COPY ./config ./config

RUN apk --update add curl imagemagick

ENV VULPIX_APP_ENVIRONMENT Production
CMD ["./vulpix"]

EXPOSE 6060
