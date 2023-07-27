## `substrate-timetravel` CLI

#### A CLI tool to extract, process and load historical state from Substrate-based chains

`substrate-timetravel` allows you to travel back in time on a substrate-based chain and explore the effects of changing the past by performing computation on historical chain states. Unfortunately, it doesn't allow to travel into future blocks, but if you figure out a way to do so, please open a PR 🧙


`substrate-timetravel` helps scrapping storage keys from remote substrate nodes and populate a local externalities that can easily be turned into snapshots for ergonomics and fast experimentation. It also provides an easy way to mutate and transform the state of externalities using pre-developed `gadgets` that can be assembled into `operations`.

Dividing the "extract" and "transform" phases offers an ergonomic way to analyse and tweak historical chain data locally.

The module `crate::gadgets` implements modular actions that a dev may find useful when inspecting and interacting with a populated externalities. The module `crate::operations` implements operations that use a set of gadgets to achieve a goal. For example, the `election_analysis` computes, among other things, election scores using different election algorithms and computes unbounded election snapshots given the state of the chain at a particular block. Those computations rely on gadgets that are modular and generic to be used by other operations.

 ## How to use the CLI

 #### 1. `substrate-timetravel extract`: Extract and store block state locally

 ```bash
  $ substrate-timetravel extract --at=<block_hash> --snapshot_path=<path> --pallets=Staking --uri=wss://rpc.polkadot.io:433
 ```

This command will fetch the block keys from a remote node, build an externalities and store its snapshot to disk for posterior analysis.

For more information and configuration options, check `substrate-timetravel extract help`.

#### 2. `substrate-timetravel transform`: Perform a transformation on a block state

```bash
 $ substrate-timetravel transform --at=<block_hash> min_active_stake --snapshot_path=<path> --uri=wss://rpc.polkadot.io:433
```

The `min_active_stake` operation will calculate the minimum active stake of a block from an externalities snapshot that has been stored under `snapshot_path`.

The advantage of splitting the `extract` from the `tranform` command is that several operations and iterations can be applied over a stored externalities snapshot without having to constantly download the block storage keys from a remote node.

The output of the operation is written in the for of a CSV file in the `output_path` (set by default as `./output.csv`).

For more information and configuration options, check `substrate-timetravel extract help`.

#### 3. Extract and transform in one command

It is possible to collapse the `extract` and `transform` into one, which is specially helpful for 1-time operations when the externalities snapshot does not yet exist. This can be achieved by using the `--live` flag with the transform command:

```bash
 $ substrate-elt transform --live --at=<block_hash> min_active_stake --snapshot_path=<path> --uri=wss://rpc.polkadot.io:433
```

The command above will 1) populate and store a remote externalities from a remote node and 2) perform the `min_active_stake` operation over that state.

## Examples

#### Fetch the minimum active stake from block

```bash
 $ cargo build
 $ RUST_LOG=info ./target/debug/substrate-timetravel transform --live --at=0x1477d54ad233824dd60afe1efc76413523c2737fd0cbabe2271568f75f560c74 min-active-stake --uri=wss://rpc.polkadot.io:443
````

The result of the operation is saved in `./output.csv` in the form of

```csv
 block_number,min_active_stake
 14401871,9517000000
```

By continuing to call `transform min-active-stake`, the results will be appended to the output file:

```csv
 block_number,min_active_stake
 14401871,9517000000
 15380091,9517000000
 14401873,9517000000
```

