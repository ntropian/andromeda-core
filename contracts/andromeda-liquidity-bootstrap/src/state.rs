use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const USERS: Map<&Addr, UserInfo> = Map::new("users");

//----------------------------------------------------------------------------------------
// Storage types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// The token address
    pub token_address: Addr,
    /// Lockdrop Contract address
    pub lockdrop_contract_address: Option<Addr>,
    ///  TOKEN-UST LP Pool address
    pub astroport_lp_pool: Option<Addr>,
    ///  TOKEN-UST LP Token address
    pub lp_token_address: Option<Addr>,
    ///  TOKEN LP Staking contract with which TOKEN-UST LP Tokens can be staked
    pub token_lp_staking_contract: Option<Addr>,
    /// Total TOKEN token rewards to be used to incentivize boostrap auction participants
    pub token_rewards: Uint128,
    /// Number of seconds over which TOKEN incentives are vested
    pub token_vesting_duration: u64,
    ///  Number of seconds over which LP Tokens are vested
    pub lp_tokens_vesting_duration: u64,
    /// Timestamp since which TOKEN / UST deposits will be allowed
    pub init_timestamp: u64,
    /// Number of seconds post init_timestamp during which UST deposits / withdrawals will be allowed
    pub ust_deposit_window: u64,
    /// Number of seconds post init_timestamp during which TOKEN delegations (via lockdrop / airdrop) will be allowed
    pub token_deposit_window: u64,
    /// Number of seconds post ust_deposit_window completion during which only partial UST withdrawals are allowed
    pub withdrawal_window: u64,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    /// Total TOKEN tokens delegated to the contract by lockdrop participants / airdrop recipients
    pub total_token_deposited: Uint128,
    /// Total UST deposited in the contract
    pub total_ust_deposited: Uint128,
    /// Total LP shares minted post liquidity addition to the TOKEN-UST Pool
    pub lp_shares_minted: Uint128,
    /// Number of LP shares that have been withdrawn as they unvest
    pub lp_shares_withdrawn: Uint128,
    /// True if TOKEN--UST LP Shares are currently staked with the TOKEN LP Staking contract
    pub are_staked_for_single_incentives: bool,
    /// True if TOKEN--UST LP Shares are currently staked with Astroport Generator for dual staking incentives
    pub are_staked_for_dual_incentives: bool,
    /// Timestamp at which liquidity was added to the TOKEN-UST LP Pool
    pub pool_init_timestamp: u64,
    /// index used to keep track of $TOKEN claimed as LP staking rewards and distribute them proportionally among the auction participants
    pub global_token_reward_index: Decimal,
    /// index used to keep track of $ASTRO claimed as LP staking rewards and distribute them proportionally among the auction participants
    pub global_astro_reward_index: Decimal,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    /// Total TOKEN Tokens delegated by the user
    pub token_deposited: Uint128,
    /// Total UST deposited by the user
    pub ust_deposited: Uint128,
    /// Withdrawal counter to capture if the user already withdrew UST during the "only withdrawals" window
    pub ust_withdrawn_flag: bool,
    /// User's LP share balance
    pub lp_shares: Uint128,
    /// LP shares withdrawn by the user
    pub withdrawn_lp_shares: Uint128,
    /// User's TOKEN rewards for participating in the auction
    pub total_auction_incentives: Uint128,
    /// TOKEN rewards withdrawn by the user
    pub withdrawn_auction_incentives: Uint128,
    /// TOKEN staking incentives (LP token staking) withdrawn by the user
    pub withdrawn_token_incentives: Uint128,
    /// ASTRO staking incentives (LP token staking) withdrawn by the user
    pub withdrawn_astro_incentives: Uint128,
    /// Index used to calculate user's $TOKEN staking rewards
    pub token_reward_index: Decimal,
    /// Index used to calculate user's $ASTRO staking rewards
    pub astro_reward_index: Decimal,
}
