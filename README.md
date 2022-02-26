
# Building

## **1. Install rustc, cargo and rustfmt.**

```bash
$ curl https://sh.rustup.rs -sSf | sh
$ source $HOME/.cargo/env
$ rustup component add rustfmt
```

When building the master branch, please make sure you are using the latest stable rust version by running:

```bash
$ rustup update
```

Note that if this is not the latest rust version on your machine, cargo commands may require an [override](https://rust-lang.github.io/rustup/overrides.html) in order to use the correct version.

On Linux systems you may need to install libssl-dev, pkg-config, zlib1g-dev, etc.  On Ubuntu:

```bash
$ sudo apt-get update
$ sudo apt-get install libssl-dev libudev-dev pkg-config zlib1g-dev llvm clang make
```

Finally, install the latest [Solana CLI Suite](https://docs.solana.com/cli/install-solana-cli-tools)

## **2. Clone the solana-keri repo.**

Using https
```bash
$ git clone https://github.com/hashblock/solana-keri
```

Using ssh
```bash
$ git clone git@github.com:hashblock/solana-keri.git
```

## **3. Build.**

First the Solana program
```bash
$ cd solana-keri/program
$ cargo build-bpf
$ cd ..
```

Then the command line wallet
```bash
$ cargo build
```

## **4. Testing**

**Run the bpf test suite:**
This runs unit tests for the Solana program (smart-contract) found in program/src/entry_point.rs. This uses a stripped down version of the solana runtime so not all features may be present

```bash
$ cargo test-bpf -- --test-threads=1 --nocapture
```

**Run the non-bpf test suite:**
This will start a local solana validator node (`solana-test-validator`)and run through tests in `src/main.rs`
```bash
$ cargo test -- --test-threads=1 --nocapture
```

## **5. Running**
WIP