# uftwo command line tool

## Using the CLI

```sh
cargo install uftwo-tools --git https://github.com/umi-eng/uftwo --locked
```

### Convert binary to UF2

```sh
uftwo convert input.bin output.uf2 --target-addr 0x08000000 --family-id 0x12345678
```

### Convert UF2 to binary

```sh
uftwo convert input.uf2 output.bin
```
