use std::fmt::{Debug, Display};

use common::ado_base::{hooks::AndromedaHook, AndromedaMsg, AndromedaQuery};
use cosmwasm_std::{Addr, Binary, Coin, CosmosMsg, StdResult, Timestamp};
use cw_storage_plus::Map;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::gatekeeper_common::UniversalMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    CreateSessionKey {
        address: String,
        max_duration: u64,
        admin_permissions: bool,
    },
    DestroySessionKey {
        address: String,
    },
    UpdateLegacyOwner {
        new_owner: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    CanExecute {
        sender: String,
        message: UniversalMsg,
    },
    AndrHook(AndromedaHook),
    AndrQuery(AndromedaQuery),
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SessionKey {
    pub address: Addr,
    pub expiration: u64,
    pub admin_permissions: bool,
}

impl SessionKey {
    pub fn is_expired(&self, current_time: Timestamp) -> bool {
        current_time.seconds() >= self.expiration
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CanExecuteResponse {
    pub can_execute: bool,
}

pub const SESSIONKEYS: Map<&Addr, SessionKey> = Map::new("sessionkeys");
