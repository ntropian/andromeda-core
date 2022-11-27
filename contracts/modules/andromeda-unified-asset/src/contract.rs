use ado_base::ADOContract;
use common::error::ContractError;
use cosmwasm_std::ensure;
#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_binary, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128,
};

use crate::constants::MAINNET_AXLUSDC_IBC;
use crate::pair_contract::PairContracts;
use crate::sourced_coin::SourcedCoin;
use crate::sources::Sources;
use crate::state::{State, STATE};
use andromeda_modules::unified_asset::{
    ExecuteMsg, InstantiateMsg, LegacyOwnerResponse, MigrateMsg, QueryMsg, UnifyAssetsMsg,
};

use cw2::{get_contract_version, set_contract_version};

use semver::Version;

use andromeda_modules::gatekeeper_common::{update_legacy_owner, LEGACY_OWNER};

// version info for migration info
const CONTRACT_NAME: &str = "obi-proxy-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    LEGACY_OWNER.save(deps.storage, &msg.legacy_owner)?;
    let mut cfg = State {
        home_network: msg.home_network,
        pair_contracts: PairContracts {
            pair_contracts: vec![],
        },
    };
    match msg.unified_price_contract {
        Some(contract) => {
            cfg.pair_contracts
            .set_pair_contracts(cfg.home_network.clone(), Some(contract))?;
        }
        None => {
            cfg.pair_contracts
            .set_pair_contracts(cfg.home_network.clone(), None)?;
        }
    }
    STATE.save(deps.storage, &cfg)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
#[allow(unused_variables)]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    // New version
    let version: Version = CONTRACT_VERSION.parse().map_err(from_semver)?;

    // Old version
    let stored = get_contract_version(deps.storage)?;
    let storage_version: Version = stored.version.parse().map_err(from_semver)?;

    let contract = ADOContract::default();

    ensure!(
        stored.contract == CONTRACT_NAME,
        ContractError::CannotMigrate {
            previous_contract: stored.contract,
        }
    );

    // New version has to be newer/greater than the old version
    ensure!(
        storage_version < version,
        ContractError::CannotMigrate {
            previous_contract: stored.version,
        }
    );

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Update the ADOContract's version
    contract.execute_update_version(deps)?;

    Ok(Response::default())
}

fn from_semver(err: semver::Error) -> StdError {
    StdError::generic_err(format!("Semver: {}", err))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateLegacyOwner { new_owner } => {
            let valid_new_owner = deps.api.addr_validate(&new_owner)?;
            update_legacy_owner(deps, info, valid_new_owner)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::LegacyOwner {} => to_binary(&query_legacy_owner(deps)?),
        QueryMsg::UnifyAssets(UnifyAssetsMsg {
            target_asset,
            assets,
            assets_are_target_amount,
        }) => to_binary(
            &unify_assets(
                deps,
                target_asset.unwrap_or_else(|| MAINNET_AXLUSDC_IBC.to_string()),
                assets,
                assets_are_target_amount,
            )
            .map_err(|e| StdError::generic_err(format!("{:?}", e)))?,
        ),
    }
}

pub fn query_legacy_owner(deps: Deps) -> StdResult<LegacyOwnerResponse> {
    let legacy_owner = LEGACY_OWNER.load(deps.storage)?;
    let legacy_owner = match legacy_owner {
        Some(legacy_owner) => legacy_owner,
        None => "No owner".to_string(),
    };
    Ok(LegacyOwnerResponse { legacy_owner })
}

pub fn unify_assets(
    deps: Deps,
    target_asset: String,
    assets: Vec<Coin>,
    assets_are_target_amount: bool,
) -> Result<SourcedCoin, ContractError> {
    let pair_contracts = STATE.load(deps.storage)?.pair_contracts;
    let mut return_coin = SourcedCoin {
        coin: Coin {
            denom: target_asset,
            amount: Uint128::zero(),
        },
        wrapped_sources: Sources { sources: vec![] },
    };
    for asset in assets {
        match asset.denom.as_str() {
            val if val == MAINNET_AXLUSDC_IBC => {
                return_coin
                    .coin
                    .amount
                    .checked_add(asset.amount)
                    .map_err(|_e| {
                        ContractError::Std(StdError::GenericErr {
                            msg: "Overflow".to_string(),
                        })
                    })?;
            }
            "ujuno" | "ujunox" | "testtokens" => {
                let unconverted = SourcedCoin {
                    coin: Coin {
                        denom: asset.denom.clone(),
                        amount: asset.amount,
                    },
                    wrapped_sources: Sources { sources: vec![] },
                };
                let mut converted = unconverted
                    .get_converted_to_usdc(deps, pair_contracts.clone(), assets_are_target_amount)
                    .map_err(|e| {
                        common::error::ContractError::Std(StdError::GenericErr {
                            msg: format!("{:?}", e),
                        })
                    })?;
                return_coin.coin.amount =
                    return_coin.coin.amount.checked_add(converted.coin.amount)?;
                return_coin
                    .wrapped_sources
                    .sources
                    .append(&mut converted.wrapped_sources.sources);
            }
            _ => {
                return Err(ContractError::Std(StdError::GenericErr {
                    msg: format!("Unknown asset {}", asset.denom),
                }))
            } // todo: more general handling
        }
    }
    Ok(return_coin)
}
