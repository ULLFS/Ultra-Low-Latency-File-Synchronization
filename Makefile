all: build
build:
	cargo xtask build
run:
	RUST_LOG=info cargo xtask run
bindings:
	${HOME}/.cargo/bin/aya-tool generate file renamedata > ./ullfs-ebpf/src/vmlinux.rs
clean:
	cargo clean
install-aya-tool:
	cargo install bindgen-cli
	cargo install --git https://github.com/aya-rs/aya -- aya-tool
	export PATH="${PATH}:${HOME}/.cargo/bin"

