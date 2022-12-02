use ado_base::ADOContract;
use common::error::ContractError;
use cosmwasm_std::{ensure};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};

use cw2::{get_contract_version, set_contract_version};

use semver::Version;

use andromeda_modules::{
    gatekeeper_common::{update_legacy_owner, UniversalMsg, LEGACY_OWNER},
    gatekeeper_spendlimit::CanSpendResponse,
    unified_asset::LegacyOwnerResponse,
    user_account::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, UserAccount, ACCOUNT},
};

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
    let account = msg.account;
    ACCOUNT.save(deps.storage, &account)?;
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
        ExecuteMsg::ProposeUpdateOwner { new_owner: _ } => todo!(),
        ExecuteMsg::ChangeOwnerUpdatesDelay { new_delay: _ } => todo!(),
        ExecuteMsg::Execute { universal_msg } => execute_execute(deps, info, universal_msg),
        ExecuteMsg::AndrReceive(_) => todo!(),
    }
}

fn execute_execute(
    deps: DepsMut,
    info: MessageInfo,
    msg: UniversalMsg,
) -> Result<Response, ContractError> {
    ensure!(
        can_execute(deps.as_ref(), info.sender.to_string(), msg.clone())?.can_spend,
        ContractError::Unauthorized {}
    );

    match msg {
        UniversalMsg::Legacy(legacy_msg) => Ok(Response::new()
            .add_attribute("execute_msg", "cosmos_msg")
            .add_message(legacy_msg)),
        UniversalMsg::Andromeda(_andromeda_msg) => {
            todo!()
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::LegacyOwner {} => {
            to_binary(&query_legacy_owner(deps)?).map_err(ContractError::Std)
        }
        QueryMsg::CanExecute {
            address,
            msg,
            funds: _,
        } => to_binary(&can_execute(deps, address, msg)?).map_err(ContractError::Std),
        QueryMsg::UpdateDelay {} => todo!(),
        QueryMsg::GatekeeperContracts {} => todo!(),
        QueryMsg::AndrHook(_) => todo!(),
        QueryMsg::AndrQuery(_) => todo!(),
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

pub fn can_execute(
    deps: Deps,
    address: String,
    msg: UniversalMsg,
) -> Result<CanSpendResponse, ContractError> {
    let account: UserAccount = ACCOUNT.load(deps.storage)?;
    let can_execute = account.can_execute(deps, address, vec![msg])?;
    Ok(can_execute)
}
