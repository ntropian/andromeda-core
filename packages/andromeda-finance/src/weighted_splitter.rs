use common::ado_base::{modules::Module, recipient::Recipient, AndromedaMsg, AndromedaQuery};
use cosmwasm_std::Uint128;
use cw_utils::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AddressWeight {
    pub recipient: Recipient,
    pub weight: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
/// A config struct for a `Splitter` contract.
pub struct Splitter {
    /// The vector of recipients for the contract. Anytime a `Send` execute message is sent the amount sent will be divided amongst these recipients depending on their assigned weight.
    pub recipients: Vec<AddressWeight>,
    /// Whether or not the contract is currently locked. This restricts updating any config related fields.
    pub lock: Expiration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    /// The vector of recipients for the contract. Anytime a `Send` execute message is
    /// sent the amount sent will be divided amongst these recipients depending on their assigned weight.
    pub recipients: Vec<AddressWeight>,
    pub lock_time: Option<u64>,
    pub modules: Option<Vec<Module>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Update the recipients list. Only executable by the contract owner when the contract is not locked.
    UpdateRecipients {
        recipients: Vec<AddressWeight>,
    },
    /// Update a specific recipient's weight. Only executable by the contract owner when the contract is not locked.
    UpdateRecipientWeight {
        recipient: AddressWeight,
    },
    /// Add a single recipient to the recipient list. Only executable by the contract owner when the contract is not locked.
    AddRecipient {
        recipient: AddressWeight,
    },
    /// Remove a single recipient from the recipient list. Only executable by the contract owner when the contract is not locked.
    RemoveRecipient {
        recipient: Recipient,
    },
    /// Used to lock/unlock the contract allowing the config to be updated.
    UpdateLock {
        lock_time: u64,
    },
    /// Divides any attached funds to the message amongst the recipients list.
    Send {},
    AndrReceive(AndromedaMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    AndrQuery(AndromedaQuery),
    /// The current config of the Splitter contract
    GetSplitterConfig {},
    /// Gets user's allocated weight
    GetUserWeight {
        user: Recipient,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct GetSplitterConfigResponse {
    pub config: Splitter,
}
/// In addition to returning a specific recipient's weight, this function also returns the total weight of all recipients.
/// This serves to put the user's weight into perspective.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
pub struct GetUserWeightResponse {
    pub weight: Uint128,
    pub total_weight: Uint128,
}
