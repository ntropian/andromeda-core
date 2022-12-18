use cosmwasm_schema::cw_serde;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cw_serde]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    KobayashiMaru { captain: String, strategy: String },
}

#[cw_serde]
pub enum QueryMsg {
    CheaterDetected {},
}

#[cw_serde]
pub struct CheaterDetectedResponse {
    pub cheater_detected: bool,
}
