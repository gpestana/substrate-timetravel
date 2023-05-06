# substrate-etl
CLI to fetch and transform data from Subtrate chains

> TODO: rust-docs and readme.md

### Fetch minimum active stake from block

```bash
$ cargo build
$ RUST_LOG=info ./target/debug/substrate-etl transform --live --at=0x1477d54ad233824dd60afe1efc76413523c2737fd0cbabe2271568f75f560c74 min-active-stake --uri=wss://rpc.polkadot.io:443
```

The result of the operation is saved in `./output.csv` in the form of

```yaml
block_number,min_active_stake
14401871,9517000000
```

(Note: the output path can be rewritten with `--output-path`, for more info check the CLI help).

You can continue to call `transform min-active-stake` and the results will be appended to the output file:

```
block_number,min_active_stake
14401871,9517000000
15380091,9517000000
14401873,9517000000
```

