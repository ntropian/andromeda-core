use std::fmt::{Display, Debug};

use common::ado_base::{hooks::AndromedaHook, AndromedaMsg, AndromedaQuery};
use cosmwasm_std::{Addr, Binary, Coin, CosmosMsg, Timestamp, StdResult};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    BeginTransaction { message: CosmosMsg, delay_seconds: u64 },
    CancelTransaction { txnumber: u64 },
    CompleteTransaction { txnumber: u64},
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    TransactionInProgress { txnumber: u64 },
    // todo: pagination
    AllTransactionsInProgress {},
    AndrHook(AndromedaHook),
    AndrQuery(AndromedaQuery),
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DelayedMsg {
    delay_expiration: u64,
    message: CosmosMsg,
}

impl DelayedMsg {
    pub fn new(delay_expiration: u64, message: CosmosMsg) -> Self {
        Self {
            delay_expiration,
            message,
        }
    }

    pub fn check_expiration(&self, current_time: Timestamp) -> StdResult<bool> {
        Ok(current_time.seconds() >= self.delay_expiration)
    }

    pub fn get_message(&self) -> CosmosMsg {
      self.message.clone()
    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TransactionResponse {
    pub delayed_transaction: DelayedMsg,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AllTransactionsResponse {
    pub transactions_with_ids: Vec<(u64, DelayedMsg)>
}
