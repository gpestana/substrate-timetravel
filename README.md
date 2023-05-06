# `substrate-etc` CLI
#### A CLI tool to extract, process and load state from Substrate-based chains

`substrate-etl` is a tool that helps scrapping storage keys from remote substrate nodes and
populate a local externalities that can be stored locally. It also providers an easy way to
mutate and transform the state of externalities using `operations` and `gadgets`.

Dividing the "extract" and "transform" phases offers a more ergonomic way to analyse and
tweak with historical chain data locally.

The module `crate::gadgets` implements modular actions that a dev may find useful when
inspecting and interacting with a populated externalities. The module `crate::operations`
implements operations that use a set of gadgets to achieve a goal. For example, the
`election_analysis` computes, among other things, election scores using different election
algorithms and computes unbounded snapshot. Those computations rely on gadgets that are
modular and generic to be used by other operations.

 ## How to use the CLI

 #### 1. `substrate-etl extract`: Extract and store block state locally

 ```bash
  $ substrate-elt extract --at=<block_hash> --snapshot_path=<path> --pallets=Staking --uri=wss://rpc.polkadot.io:433
 ```

 This command will fetch the block keys from a remote node, build an externalities and store its
 snapshot to disk for posterior analysis.

 For more information and configuration options, check `substrate-etl extract help`.

 #### 2. `substrate-etl transform`: Perform a transformation on a block state

 ```bash
  $ substrate-elt transform --at=<block_hash> min_active_stake --snapshot_path=<path> --uri=wss://rpc.polkadot.io:433
 ```

 The `min_active_stake` operation will calculate the minimum active stake of a block which
 externalities snapshot has been stored under the snapshot_path.

 The advantage of splitting the `extract` from the `tranform` command is that several operations
 and iterations can be applied over a stored externalities snapshot without having to constantly
 download the block storage keys from a remote node.

 The output of the operation is written in the for of a CSV file in the `output_path` (set by
 default as `./output.csv`).

 For more information and configuration options, check `substrate-etl extract help`.

 #### 3.Extract and transform in one command

 It is possible to collapse the `extract` and `transform` into one, which is specially helpful
 for 1-time operations when the externalities snapshot does not yet exist. This can be achieved
 by using the `--live` flag with the transform command:

 ```bash
  $ substrate-elt transform --live --at=<block_hash> min_active_stake --snapshot_path=<path> --uri=wss://rpc.polkadot.io:433
 ```

 The command above will 1) populate and store a remote externalities from a remote node and
 2) perform the `min_active_stake` operation over that state.

 ## Examples

 #### Fetch the minimum active stake from block

 ```bash
  $ cargo build
  $ RUST_LOG=info ./target/debug/substrate-etl transform --live --at=0x1477d54ad233824dd60afe1efc76413523c2737fd0cbabe2271568f75f560c74 min-active-stake --uri=wss://rpc.polkadot.io:443
 ````
 The result of the operation is saved in `./output.csv` in the form of

 ```csv
 block_number,min_active_stake
 14401871,9517000000
 ```
 You can continue to call transform min-active-stake and the results will be appended to the output file:

 ```csv
 block_number,min_active_stake
 14401871,9517000000
 15380091,9517000000
 14401873,9517000000
 ```

