use common::{ado_base::AndromedaMsg, app::AndrAddress};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;
use cw_asset::AssetInfo;
use serde::{Deserialize, Serialize};

#[cw_serde]
pub struct InstantiateMsg {
    token_address: AndrAddress,
}

#[cw_serde]
pub enum ExecuteMsg {
    CancelSale { asset: AssetInfo },
    Purchase { recipient: Option<String> },
    Receive(Cw20ReceiveMsg),
    AndrReceive(AndromedaMsg),
}

#[derive(Deserialize, Serialize)]
pub enum CW20HookMsg {
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
    #[returns(ExchangeRateResponse)]
    ExchangeRate { asset: AssetInfo },
}

#[cw_serde]
pub struct ExchangeRateResponse {
    rate: Option<Uint128>,
}

#[cw_serde]
pub struct MigrateMsg {}
