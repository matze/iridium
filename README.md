# Iridium

Iridium is a [Standard Notes](https://standardnotes.org) client written in Rust
and GTK. It synchronizes with any compliant Standard Notes server but can work
entirely offline as well.

![main window](https://i.imgur.com/F2E8KFs.png)

## Building from source

Iridium is written in Rust, so you will need the Rust toolchain. You could use
[rustup](https://rustup.rs) to install update Rust and Cargo. Then build, test and run
with

    $ cargo build --release
    $ cargo test --release
    $ cargo run --release

To display logs during execution set the `RUST_LOG` environment variable, e.g.
to display debug logs run via

    $ RUST_LOG=debug cargo run
