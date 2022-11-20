use andromeda_modules::gatekeeper_delay::DelayedMsg;
use cosmwasm_std::{CosmosMsg, Deps, DepsMut, Env, StdResult, Timestamp};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const QUEUE: Map<&[u8], DelayedMsg> = Map::new("queue");
pub const COUNTER: Item<u64> = Item::new("counter");
pub const EXPIRATION: Item<u64> = Item::new("expiration");

pub fn next_id(deps: DepsMut) -> StdResult<u64> {
    let ct = COUNTER.load(deps.storage)?.wrapping_add(1u64);
    COUNTER.save(deps.storage, &ct)?;
    Ok(ct)
}
