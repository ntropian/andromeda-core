use andromeda_modules::gatekeeper_delay::DelayedMsg;
use cosmwasm_std::{DepsMut, StdResult};
use cw_storage_plus::{Item, Map};

pub const QUEUE: Map<&[u8], DelayedMsg> = Map::new("queue");
pub const COUNTER: Item<u64> = Item::new("counter");

pub fn next_id(deps: DepsMut) -> StdResult<u64> {
    let ct = COUNTER.load(deps.storage)?.wrapping_add(1u64);
    COUNTER.save(deps.storage, &ct)?;
    Ok(ct)
}
