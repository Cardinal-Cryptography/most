# *most* e2e test suite

This crate allows to run e2e tests for *most*.

## Usage

In order to use the e2e test suite, we need to have *most* running in the background. In particular, we have to have running:

- An *Aleph Zero* chain.
- An *Ethereum* chain.
- All the relevant contracts deployed on both chains.
- A committee of *guardians*.
- A *relayer* for communication between the chains.

Running one specific test using default config values:
```bash
$ RUST_LOG=info cargo test test::test_module::test_name -- --color always --exact --nocapture
```

There is also a convenience `make` instruction to run with default parameters:
```bash
$ make e2e-tests TEST_CASE=test_module::test_name
```
