use cosmwasm_std::{Addr, StdResult, Storage, Uint128};
use cw_storage_plus::{Index, IndexList, IndexedMap, MultiIndex};
use cw_storage_plus::{Item, UniqueIndex};

use andromeda_modules::gatekeeper::Authorization;

pub struct AuthorizationIndexes<'a> {
    // pk goes to second tuple element
    pub identifier: UniqueIndex<'a, u16, Authorization>,
    pub actor: MultiIndex<'a, Addr, Authorization, Vec<u8>>,
    pub contract: MultiIndex<'a, Addr, Authorization, Vec<u8>>,
    pub message_name: MultiIndex<'a, String, Authorization, Vec<u8>>,
    pub wasmaction_name: MultiIndex<'a, String, Authorization, Vec<u8>>,
}

impl<'a> IndexList<Authorization> for AuthorizationIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Authorization>> + '_> {
        let v: Vec<&dyn Index<Authorization>> = vec![
            &self.identifier,
            &self.actor,
            &self.contract,
            &self.message_name,
            &self.wasmaction_name,
        ];
        Box::new(v.into_iter())
    }
}

const AUTH_COUNT_KEY: &str = "auth_count";

const AUTHORIZATIONS_KEY: &str = "auths";
const AUTHORIZATIONS_UNIQUE_KEYS: &str = "auths__unique_key";
const AUTHORIZATIONS_ACTORS_KEY: &str = "auths__actors";
const AUTHORIZATIONS_CONTRACTS_KEY: &str = "auths__contracts";
const AUTHORIZATIONS_MESSAGE_NAMES_KEY: &str = "auths__message_names";
const AUTHORIZATIONS_WASMACTION_NAMES_KEY: &str = "auths__wasmaction_names";

pub fn authorizations<'a>() -> IndexedMap<'a, &'a [u8], Authorization, AuthorizationIndexes<'a>> {
    IndexedMap::new(
        AUTHORIZATIONS_KEY,
        AuthorizationIndexes {
            identifier: UniqueIndex::new(|d| d.identifier, AUTHORIZATIONS_UNIQUE_KEYS),
            actor: MultiIndex::new(
                |d| (d.actor.clone().unwrap()),
                AUTHORIZATIONS_KEY,
                AUTHORIZATIONS_ACTORS_KEY,
            ),
            contract: MultiIndex::new(
                |d| (d.contract.clone().unwrap()),
                AUTHORIZATIONS_KEY,
                AUTHORIZATIONS_CONTRACTS_KEY,
            ),
            message_name: MultiIndex::new(
                |d| (d.message_name.clone().unwrap()),
                AUTHORIZATIONS_KEY,
                AUTHORIZATIONS_MESSAGE_NAMES_KEY,
            ),
            wasmaction_name: MultiIndex::new(
                |d| (d.wasmaction_name.clone().unwrap()),
                AUTHORIZATIONS_KEY,
                AUTHORIZATIONS_WASMACTION_NAMES_KEY,
            ),
        },
    )
}

pub const COUNTER: Item<Uint128> = Item::new(AUTH_COUNT_KEY);
pub const OWNER: Item<Addr> = Item::new("local_owner");

pub fn is_owner(storage: &dyn Storage, address: String) -> StdResult<bool> {
    let owner = OWNER.load(storage)?;
    Ok(address == owner)
}
