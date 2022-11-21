use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::pair_contract::PairContracts;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct State {
    pub home_network: String,
    pub pair_contracts: PairContracts,
}

pub const STATE: Item<State> = Item::new("state");
