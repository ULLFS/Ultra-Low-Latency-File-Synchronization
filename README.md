# ullfs

## Prerequisites

1. Install bpf-linker: `cargo install bpf-linker`

## Build eBPF

```bash
cargo xtask build-ebpf
```

To perform a release build you can use the `--release` flag.
You may also change the target architecture with the `--target` flag.

## Build Userspace

```bash
cargo build
```

## Build eBPF and Userspace

```bash
cargo xtask build
```

## Run

```bash
RUST_LOG=info cargo xtask run
```


## Config Options:

```json
{
    "watch_dir":"<The full directory to be watched, can not be a local directory or use ~",
    "ignore_ext":["<A list of extensions to ignore>"],
    "ignore_subdirs":["<A list of sub directories to ignore, these should be formatted to not include the watch_dir. Start these directories with no slash. Must be full subdirectory>"],
    "ignore_files":["A list of specific files, use full path for each of these, to ignore."],
    "32_bit_inodes":false // This should be false on systems with 64 bit inodes. Set to true if file changes are not being detected
}
```