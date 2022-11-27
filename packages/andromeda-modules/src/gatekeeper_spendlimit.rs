use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, Coin, CosmosMsg, Uint128};

use crate::permissioned_address::{CoinLimit, PermissionedAddressParams};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub legacy_owner: Option<String>,
    pub permissioned_addresses: Vec<PermissionedAddressParams>,
    pub asset_unifier_contract: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Proposes a new owner for the proxy contract – must be called by the existing owner
    UpdateLegacyOwner { new_owner: String },
    UpsertBeneficiary {
        new_beneficiary: PermissionedAddressParams,
    },
    UpsertPermissionedAddress {
        new_permissioned_address: PermissionedAddressParams,
    },
    /// Removes an active spend-limited wallet. This includes beneficiaries.
    RmPermissionedAddress { doomed_permissioned_address: String },
    /// Updates spend limit for a wallet. Update of period not supported: rm and re-add
    UpdatePermissionedAddressSpendLimit {
        permissioned_address: String,
        new_spend_limits: CoinLimit,
        is_beneficiary: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Shows owner; always mutable
    LegacyOwner {},
    PermissionedAddresss {},
    /// Returns true if address 1) is admin, 2) is permissioned address and msg is spendable
    /// by permissioned address, or 3) is one of approved cw20s (no funds attached tho)
    CanSpend {
        sender: String,
        funds: Vec<Coin>,
        msgs: Vec<CosmosMsg>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WasmExecuteMsg {
    contract_addr: String,
    /// msg is the json-encoded ExecuteMsg struct (as raw Binary)
    pub msg: Binary,
    funds: Vec<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct TestExecuteMsg {
    pub foo: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct TestFieldsExecuteMsg {
    pub recipient: String,
    pub strategy: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct CanSpendResponse {
    pub can_spend: bool,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct UpdateDelayResponse {
    pub update_delay_hours: u16,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub enum Cw20ExecuteMsg {
    Transfer { recipient: String, amount: Uint128 },
}
