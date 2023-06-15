FROM rust:1.70 as build

RUN USER=root cargo new --bin vulpix
WORKDIR /vulpix

ENV MAGICK_VERSION 7.1

RUN apt-get update \
 && apt-get -y install curl build-essential clang pkg-config libjpeg-turbo-progs libpng-dev \
 && rm -rfv /var/lib/apt/lists/*

RUN curl https://imagemagick.org/archive/ImageMagick.tar.gz | tar xz \
 && cd ImageMagick-${MAGICK_VERSION}* \
 && ./configure --with-magick-plus-plus=no --with-perl=no \
 && make \
 && make install \
 && cd .. \
 && rm -r ImageMagick-${MAGICK_VERSION}*

ENV LD_LIBRARY_PATH=/usr/local/lib

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release

RUN rm src/*.rs
COPY ./src ./src

RUN rm ./target/release/deps/vulpix*
RUN cargo build --release

FROM rust:1.70
COPY --from=build /vulpix/target/release/vulpix .

RUN apt-get update \
 && apt-get -y install curl build-essential clang pkg-config libjpeg-turbo-progs libpng-dev \
 && rm -rfv /var/lib/apt/lists/*

RUN curl https://imagemagick.org/archive/ImageMagick.tar.gz | tar xz \
 && cd ImageMagick-${MAGICK_VERSION}* \
 && ./configure --with-magick-plus-plus=no --with-perl=no \
 && make \
 && make install \
 && cd .. \
 && rm -r ImageMagick-${MAGICK_VERSION}*

ENV LD_LIBRARY_PATH=/usr/local/lib

CMD ["./vulpix"]

EXPOSE 6060
