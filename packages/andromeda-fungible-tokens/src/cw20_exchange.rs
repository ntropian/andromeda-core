use common::{
    ado_base::{AndromedaMsg, AndromedaQuery},
    app::AndrAddress,
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;
use cw_asset::AssetInfo;
use serde::{Deserialize, Serialize};

#[cw_serde]
pub struct InstantiateMsg {
    /// Address of the CW20 token to be sold
    pub token_address: AndrAddress,
}

#[cw_serde]
pub enum ExecuteMsg {
    CancelSale { asset: AssetInfo },
    Purchase { recipient: Option<String> },
    Receive(Cw20ReceiveMsg),
    AndrReceive(AndromedaMsg),
}

#[cw_serde]
pub struct Sale {
    pub exchange_rate: Uint128,
    pub amount: Uint128,
}

#[derive(Deserialize, Serialize)]
pub enum Cw20HookMsg {
    StartSale {
        asset: AssetInfo,
        exchange_rate: Uint128,
    },
    Purchase {
        recipient: Option<String>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(SaleResponse)]
    Sale { asset: AssetInfo },
    #[returns(AndromedaQuery)]
    AndrQuery(AndromedaQuery),
}

#[cw_serde]
pub struct SaleResponse {
    pub sale: Option<Sale>,
}

#[cw_serde]
pub struct MigrateMsg {}
