use common::ado_base::{hooks::AndromedaHook, AndromedaMsg, AndromedaQuery};
use cosmwasm_std::{Addr, Binary, Coin, CosmosMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::gatekeeper_common::UniversalMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Authorization {
    pub identifier: u16,
    pub actor: Option<Addr>,
    pub contract: Option<Addr>,
    pub message_name: Option<String>,
    pub wasmaction_name: Option<String>,
    pub fields: Option<Vec<(String, String)>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    AddAuthorization {
        new_authorization: Authorization,
    },
    RemoveAuthorization {
        authorization_to_remove: Authorization,
    },
    RmAllMatchingAuthorizations {
        authorization_to_remove: Authorization,
    },
    UpdateLegacyOwner {
        new_owner: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WasmExecuteMsg {
    contract_addr: String,
    /// msg is the json-encoded ExecuteMsg struct (as raw Binary)
    pub msg: Binary,
    funds: Vec<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // Returns authorizations filtered by whatever is not None
    Authorizations {
        identifier: Option<u16>, // overrides all
        actor: Option<String>,
        target_contract: Option<String>,
        message_name: Option<String>,
        wasmaction_name: Option<String>,
        fields: Option<Vec<(String, String)>>,
        limit: Option<u32>,
        start_after: Option<String>,
    },
    // Check whether specific message(s) is/are authorized
    CheckTransaction {
        msg: UniversalMsg,
        sender: String,
    },
    AndrHook(AndromedaHook),
    AndrQuery(AndromedaQuery),
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AuthorizationsResponse {
    pub authorizations: Vec<(Vec<u8>, Authorization)>,
}
