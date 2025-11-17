# MHWs Tex Decompressor

A tool to make a pak with uncompressed textures for MHWilds.

## Usage

### Windows

1. Download from [Releases](https://github.com/eigeen/mhws-tex-decompressor/releases).
2. If it is a zip file, extract it.
3. Run exe file, follow the instructions.

### Arch Linux

```sh 
git clone https://github.com/eigeen/mhws-tex-decompressor
cd mhws-tex-decompressor
sudo pacman -S cargo rustc libssl-dev pkg-config
cargo run . --release
```

### Ubuntu 25.10 and older

```sh 
git clone https://github.com/eigeen/mhws-tex-decompressor
cd mhws-tex-decompressor
sudo add-apt-repository ppa:maxgmr/rustc-1.88-merge
sudo apt install cargo-1.88 rustc-1.88 libssl-dev pkg-config
sudo ln -s /usr/bin/cargo-1.88 /usr/bin/cargo
sudo ln -s /usr/bin/rustc-1.88 /usr/bin/rustc
cargo run . --release
```

#### Nix

``` sh
nix run github:eigeen/mhws-tex-decompressor
```

## Credits

[@AsteriskAmpersand](https://github.com/AsteriskAmpersand)
