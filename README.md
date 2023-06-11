# Vulpix

An image processing service.

## setup

install from [rust](https://www.rust-lang.org/tools/install) 
or 

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

install image magick

```bash
brew install imagemagick
```

aws env variables

```
AWS_ACCESS_KEY_ID=
AWS_SECRET_ACCESS_KEY=
AWS_REGION=
```

## dev 

```bash
cargo install cargo-watch
```

```bash
cargo watch -q -c -w src/ -x run
```

## build

```bash
cargo build --release
```