use ado_base::ADOContract;
use andromeda_modules::gatekeeper_common::{is_legacy_owner, update_legacy_owner, LEGACY_OWNER};
use andromeda_modules::permissioned_address::{
    CoinLimit, PermissionedAddress, PermissionedAddressParams, PermissionedAddresssResponse,
};
use andromeda_modules::sourced_coin::SourcedCoins;
use andromeda_modules::sources::Sources;
use andromeda_modules::unified_asset::LegacyOwnerResponse;
use cosmwasm_std::{ensure, Api};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Response, StakingMsg, StdError, StdResult, WasmMsg,
};

use crate::error::ContractError as CustomError;
use crate::state::{State, STATE};
use andromeda_modules::gatekeeper_spendlimit::{
    CanSpendResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use common::error::ContractError;
use cw2::{get_contract_version, set_contract_version};
use semver::Version;

// version info for migration info
const CONTRACT_NAME: &str = "obi-proxy-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// temporary
const ASSET_UNIFIER_CONTRACT_ADDRESS: &str = "LOCAL_TEST";

pub struct SourcedRepayMsg {
    pub repay_msg: Option<BankMsg>,
    pub wrapped_sources: Sources,
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    LEGACY_OWNER.save(deps.storage, &msg.legacy_owner)?;
    let cfg = State {
        permissioned_addresses: msg
            .permissioned_addresses
            .into_iter()
            .map(|params| PermissionedAddress::new(params, false))
            .collect::<Vec<PermissionedAddress>>(),
        asset_unifier_contract: msg.asset_unifier_contract,
    };
    STATE.save(deps.storage, &cfg)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
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

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    // temporarily mapping these errors over
    match msg {
        ExecuteMsg::UpsertBeneficiary { new_beneficiary } => {
            upsert_permissioned_address(deps, env, info, new_beneficiary, true)
                .map_err(|e| ContractError::Std(StdError::GenericErr { msg: e.to_string() }))
        }
        ExecuteMsg::UpsertPermissionedAddress {
            new_permissioned_address,
        } => upsert_permissioned_address(deps, env, info, new_permissioned_address, false)
            .map_err(|e| ContractError::Std(StdError::GenericErr { msg: e.to_string() })),
        ExecuteMsg::RmPermissionedAddress {
            doomed_permissioned_address,
        } => rm_permissioned_address(deps, env, info, doomed_permissioned_address)
            .map_err(|e| ContractError::Std(StdError::GenericErr { msg: e.to_string() })),
        ExecuteMsg::UpdatePermissionedAddressSpendLimit {
            permissioned_address,
            new_spend_limits,
            is_beneficiary,
        } => update_permissioned_address_spend_limit(
            deps,
            env,
            info,
            permissioned_address,
            new_spend_limits,
            is_beneficiary,
        )
        .map_err(|e| ContractError::Std(StdError::GenericErr { msg: e.to_string() })),
        ExecuteMsg::UpdateLegacyOwner { new_owner } => {
            let valid_new_owner = deps.api.addr_validate(&new_owner)?;
            update_legacy_owner(deps, info, valid_new_owner)
        }
        ExecuteMsg::AndrReceive(msg) => {
            ADOContract::default().execute(deps, env, info, msg, execute)
        }
    }
}

pub fn upsert_permissioned_address(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_permissioned_address_params: PermissionedAddressParams,
    beneficiary: bool,
) -> Result<Response, CustomError> {
    let mut cfg = STATE.load(deps.storage)?;
    ensure!(
        is_legacy_owner(deps.as_ref(), info.sender.clone())?
            || ADOContract::default()
                .is_owner_or_operator(deps.storage, info.sender.as_str())
                .map_err(|e| CustomError::CustomError {
                    val: format!("ADO error, loc 1: {}", e)
                })?,
        CustomError::Unauthorized {}
    );
    if cfg
        .permissioned_addresses
        .iter()
        .any(|wallet| wallet.address() == Some(new_permissioned_address_params.address.clone()))
    {
        Err(CustomError::PermissionedAddressExists {})
    } else {
        let _addrcheck = deps
            .api
            .addr_validate(&new_permissioned_address_params.address)?;
        cfg.upsert_permissioned_address(new_permissioned_address_params, beneficiary);
        STATE.save(deps.storage, &cfg)?;
        Ok(Response::new().add_attribute("action", "add_permissioned_address"))
    }
}

pub fn rm_permissioned_address(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    doomed_permissioned_address: String,
) -> Result<Response, CustomError> {
    let mut cfg = STATE.load(deps.storage)?;
    ensure!(
        is_legacy_owner(deps.as_ref(), info.sender.clone())?
            || ADOContract::default()
                .is_owner_or_operator(deps.storage, info.sender.as_str())
                .map_err(|e| CustomError::CustomError {
                    val: format!("ADO error, loc 2: {}", e)
                })?,
        CustomError::Unauthorized {}
    );
    if !cfg
        .permissioned_addresses
        .iter()
        .any(|wallet| wallet.address() == Some(doomed_permissioned_address.clone()))
    {
        Err(CustomError::PermissionedAddressDoesNotExist {})
    } else {
        cfg.rm_permissioned_address(doomed_permissioned_address);
        STATE.save(deps.storage, &cfg)?;
        Ok(Response::new().add_attribute("action", "rm_permissioned_address"))
    }
}

pub fn update_permissioned_address_spend_limit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    permissioned_address: String,
    new_spend_limits: CoinLimit,
    is_beneficiary: String,
) -> Result<Response, CustomError> {
    let mut cfg = STATE.load(deps.storage)?;
    ensure!(
        is_legacy_owner(deps.as_ref(), info.sender.clone())?
            || ADOContract::default()
                .is_owner_or_operator(deps.storage, info.sender.as_str())
                .map_err(|e| CustomError::CustomError {
                    val: format!("ADO error, loc 3: {}", e)
                })?,
        CustomError::Unauthorized {}
    );
    let wallet = cfg
        .permissioned_addresses
        .iter_mut()
        .find(|wallet| wallet.address() == Some(permissioned_address.clone()))
        .ok_or(CustomError::PermissionedAddressDoesNotExist {})?;
    wallet.update_spend_limit(new_spend_limits, is_beneficiary)?;
    STATE.save(deps.storage, &cfg)?;
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, CustomError> {
    match msg {
        QueryMsg::PermissionedAddresss {} => to_binary(&query_permissioned_addresses(deps)?)
            .map_err(|e| CustomError::CustomError { val: e.to_string() }),
        QueryMsg::CanSpend { sender, funds } => to_binary(&query_can_spend(
            deps,
            env,
            sender,
            funds,
            STATE.load(deps.storage)?.asset_unifier_contract,
        )?)
        .map_err(|e| CustomError::CustomError { val: e.to_string() }),
        QueryMsg::LegacyOwner {} => to_binary(&LegacyOwnerResponse {
            legacy_owner: LEGACY_OWNER
                .load(deps.storage)?
                .unwrap_or_else(|| "No legacy owner".to_string()),
        })
        .map_err(|e| CustomError::CustomError { val: e.to_string() }),
    }
}

pub fn query_permissioned_addresses(deps: Deps) -> StdResult<PermissionedAddresssResponse> {
    let cfg = STATE.load(deps.storage)?;
    Ok(PermissionedAddresssResponse {
        permissioned_addresses: cfg
            .permissioned_addresses
            .into_iter()
            // temporary: unsafe unwrap
            .map(|wallet| wallet.get_params_clone().unwrap())
            .collect(),
    })
}

pub fn query_can_spend(
    deps: Deps,
    env: Env,
    sender: String,
    funds: Vec<Coin>,
    asset_unifier_contract_address: String,
) -> Result<CanSpendResponse, CustomError> {
    Ok(can_spend(deps, env, sender, funds, asset_unifier_contract_address)?.0)
}

pub fn check_owner(deps: Deps, sender: String) -> bool {
    if let Ok(check1) = ADOContract::default()
        .is_owner_or_operator(deps.storage, sender.as_str())
        .map_err(|e| CustomError::CustomError {
            val: format!("ADO error, loc 4: {}", e),
        })
    {
        return check1;
    } else if let Ok(check2) = deps.api.addr_validate(&sender) {
        if let Ok(check3) = is_legacy_owner(deps, check2) {
            return check3;
        }
    }
    false
}

pub fn can_spend(
    deps: Deps,
    env: Env,
    sender: String,
    funds: Vec<Coin>,
    asset_unifier_contract_address: String,
) -> Result<(CanSpendResponse, Option<SourcedCoins>), CustomError> {
    let cfg = STATE.load(deps.storage)?;
    let res = cfg.check_spend_limits(
        deps,
        asset_unifier_contract_address,
        env.block.time,
        sender,
        funds,
    );
    match res {
        Ok(coin) => Ok((
            CanSpendResponse {
                can_spend: true,
                reason: "Permissioned address, with spending within spend limits".to_string(),
            },
            Some(coin),
        )),
        Err(_) => Ok((
            CanSpendResponse {
                can_spend: false,
                reason: "Permissioned address does not exist or over spend limit".to_string(),
            },
            None,
        )),
    }
}

pub fn maybe_addr(api: &dyn Api, human: Option<String>) -> StdResult<Option<Addr>> {
    human.map(|x| api.addr_validate(&x)).transpose()
}
