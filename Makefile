prog := termusic 
daemon := termusicd 

default: fmt 

fmt:
	cargo fmt --all
	cargo check --all
	cargo clippy --all
	# cargo clippy -- -D warnings

run: 
	cargo run --all 

release:
	cargo build --release --all

m: 
	cargo build --features mpris --release --all

c: 
	cargo build --features cover --release --all

f:
	cargo build --features mpris,cover,discord --release --all
	# cargo build --features mpris,cover --release

mpv:
	cargo build --no-default-features --features mpris,cover,mpv --release --all

gst:
	cargo build --no-default-features --features mpris,cover,gst --release --all

mpris: m post

cover: c post

full: f post
# full: mpv post
# full: gst post

minimal: release post

post:
	mkdir -p ~/.local/share/cargo/bin/
	cp -f target/release/$(prog) ~/.local/share/cargo/bin/
	cp -f target/release/$(daemon) ~/.local/share/cargo/bin/

install: release post





