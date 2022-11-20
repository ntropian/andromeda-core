use common::{ado_base::AndromedaMsg, error::ContractError};
use cosmwasm_std::{Addr, CosmosMsg, Deps, DepsMut, MessageInfo, Response, StdResult};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    /// `legacy_owner` is for use when not in an Andromeda context.
    /// Otherwise, use e.g. .execute_update_operators()
    pub legacy_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum UniversalMsg {
    Andromeda(AndromedaMsg),
    Legacy(CosmosMsg),
}

impl std::fmt::Display for UniversalMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub const LEGACY_OWNER: Item<Option<String>> = Item::new("legacy_owner");

pub fn is_legacy_owner(deps: Deps, addy: Addr) -> StdResult<bool> {
    Ok(Some(addy.to_string()) == LEGACY_OWNER.load(deps.storage)?)
}

pub fn update_legacy_owner(
    deps: DepsMut,
    info: MessageInfo,
    addy: Addr,
) -> Result<Response, ContractError> {
    assert!(is_legacy_owner(deps.as_ref(), info.sender)?);
    LEGACY_OWNER.save(deps.storage, &Some(addy.to_string()))?;
    Ok(Response::default().add_attribute("action", "update_legacy_owner"))
}

// For unit tests
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestMsg {
    KesselRun(TestExecuteMsg),
    KobayashiMaru(TestFieldsExecuteMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct TestExecuteMsg {
    pub parsecs: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct TestFieldsExecuteMsg {
    pub recipient: String,
    pub strategy: String,
}
