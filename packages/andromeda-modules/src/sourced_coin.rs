use common::error::ContractError;
use cosmwasm_std::{to_binary, Attribute, Coin, Deps, QueryRequest, StdError, Uint128, WasmQuery};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    permissioned_address::JUNO_MAINNET_AXLUSDC_IBC,
    sources::{Source, Sources},
    unified_asset::{UnifiedAssetsResponse, UnifyAssetsMsg},
};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct SourcedCoins {
    pub coins: Vec<Coin>,
    pub wrapped_sources: Sources,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub enum UnifyAssetsQueryMsg {
    UnifyAssets(UnifyAssetsMsg),
}

pub fn get_admin_sourced_coin() -> SourcedCoins {
    SourcedCoins {
        coins: vec![Coin {
            denom: String::from("unlimited"),
            amount: Uint128::from(0u128),
        }],
        wrapped_sources: Sources {
            sources: [Source {
                contract_addr: String::from("no spend limit check"),
                query_msg: String::from("caller is admin"),
            }]
            .to_vec(),
        },
    }
}

impl SourcedCoins {
    pub fn convert_to_usdc(
        &self,
        deps: Deps,
        asset_unifier_contract_address: String,
        _amount_is_target: bool,
    ) -> Result<UnifiedAssetsResponse, ContractError> {
        let query_msg: UnifyAssetsQueryMsg = UnifyAssetsQueryMsg::UnifyAssets(UnifyAssetsMsg {
            target_asset: Some(JUNO_MAINNET_AXLUSDC_IBC.to_string()),
            assets: self.coins.clone(),
            assets_are_target_amount: false, //_amount_is_target
        });
        #[cfg(test)]
        // local single contract test uses test assets worth 100 USDC each
        if asset_unifier_contract_address == "LOCAL_TEST".to_string() {
            return Ok(UnifiedAssetsResponse {
                unified_asset: Coin {
                    denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
                    amount: self.coins[0].amount.checked_mul(Uint128::from(100u128))?,
                },
                sources: Sources { sources: vec![] },
            });
        } else {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: "local test should use LOCAL_TEST as contract".to_string(),
            }));
        }
        #[cfg(not(test))]
        let query_response: UnifiedAssetsResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: asset_unifier_contract_address,
                msg: to_binary(&query_msg)?,
            }))?;
        #[cfg(not(test))]
        Ok(query_response)
    }

    pub fn sources_as_attributes(&self) -> Vec<Attribute> {
        let mut attributes: Vec<Attribute> = vec![];
        for n in 0..self.wrapped_sources.sources.len() {
            attributes.push(Attribute {
                key: format!(
                    "query to contract {}",
                    self.wrapped_sources.sources[n].contract_addr.clone()
                ),
                value: self.wrapped_sources.sources[n].query_msg.clone(),
            })
        }
        attributes
    }
}
