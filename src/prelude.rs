#![allow(unused)]

/// The account id type.
pub type AccountId = core_primitives::AccountId;
/// The block number type.
pub type BlockNumber = core_primitives::BlockNumber;
/// The balance type.
pub type Balance = core_primitives::Balance;
/// The index of an account.
pub type Index = core_primitives::AccountIndex;
/// The hash type. We re-export it here, but we can easily get it from block as well.
pub type Hash = core_primitives::Hash;
/// The header type. We re-export it here, but we can easily get it from block as well.
pub type Header = core_primitives::Header;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub use sp_runtime::traits::{Block as BlockT, Header as HeaderT};

/// Default URI to connect to.
pub const DEFAULT_URI: &str = "wss://rpc.polkadot.io:443";
/// The logging target.
pub const LOG_TARGET: &str = "substrate_timetravel";

/// The election provider pallet.
pub use pallet_election_provider_multi_phase as EPM;

// The staking pallet.
pub use pallet_staking as Staking;

pub use pallet_bags_list as BagsList;

/// The externalities type.
pub type Ext = sp_io::TestExternalities;

/// The key pair type being used. We "strongly" assume sr25519 for simplicity.
pub type Pair = sp_core::sr25519::Pair;

/// A dynamic token type used to represent account balances.
pub type Token = sub_tokens::dynamic::DynamicToken;
