# Vulpix

An image processing service.

## setup

install from [rust](https://www.rust-lang.org/tools/install) 
or 

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
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