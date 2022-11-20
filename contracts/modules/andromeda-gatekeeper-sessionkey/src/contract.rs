

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdError,
};
use cw2::{get_contract_version, set_contract_version};



use ado_base::ADOContract;
use andromeda_modules::gatekeeper_common::{
    is_legacy_owner, update_legacy_owner, InstantiateMsg, UniversalMsg, LEGACY_OWNER,
};
use andromeda_modules::gatekeeper_sessionkey::{
    CanExecuteResponse, ExecuteMsg, MigrateMsg, QueryMsg, SessionKey, SESSIONKEYS,
};

use common::{
    ado_base::{hooks::AndromedaHook, AndromedaQuery, InstantiateMsg as BaseInstantiateMsg},
    encode_binary,
    error::ContractError,
};

use semver::Version;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-addresslist";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    LEGACY_OWNER.save(deps.storage, &msg.legacy_owner)?;
    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "address-list".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            modules: None,
            primitive_contract: None,
        },
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AndrReceive(msg) => {
            ADOContract::default().execute(deps, env, info, msg, execute)
        }
        ExecuteMsg::CreateSessionKey {
            address,
            max_duration,
            admin_permissions,
        } => create_session_key(deps, env, info, address, max_duration, admin_permissions),
        // note that session key can always destroy itself, even without admin/gatekeeper permission
        ExecuteMsg::DestroySessionKey { address } => destroy_session_key(deps, env, info, address),
        ExecuteMsg::UpdateLegacyOwner { new_owner } => {
            let valid_new_owner = deps.api.addr_validate(&new_owner)?;
            update_legacy_owner(deps, info, valid_new_owner)
        }
    }
}

fn create_session_key(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    max_duration: u64,
    admin_permissions: bool,
) -> Result<Response, ContractError> {
    let valid_address = deps.api.addr_validate(&address)?;
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.as_ref().storage, info.sender.as_str())?
            || is_legacy_owner(deps.as_ref(), info.sender)?,
        ContractError::Unauthorized {}
    );
    // if exists, can be updated ... but session key should probably not be able to refresh itself
    let new_session_key = SessionKey {
        address: deps.api.addr_validate(&address)?,
        expiration: env.block.time.seconds().saturating_add(max_duration),
        admin_permissions,
    };
    SESSIONKEYS
        .update(deps.storage, &valid_address, |_| Ok(new_session_key))
        .map_err(ContractError::Std)?;
    Ok(Response::new()
        .add_attribute("action", "create_session_key")
        .add_attribute("address", address))
}

fn destroy_session_key(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let valid_address = deps.api.addr_validate(&address)?;
    let session_key = SESSIONKEYS.load(deps.storage, &valid_address)?;
    ensure!(
        session_key.address == info.sender
            || ADOContract::default()
                .is_owner_or_operator(deps.as_ref().storage, info.sender.as_str())?
            || is_legacy_owner(deps.as_ref(), info.sender)?,
        ContractError::Unauthorized {}
    );
    SESSIONKEYS.remove(deps.storage, &valid_address);
    Ok(Response::new()
        .add_attribute("action", "destroy_session_key")
        .add_attribute("address", address))
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrHook(msg) => handle_andr_hook(deps, msg),
        QueryMsg::AndrQuery(msg) => handle_andromeda_query(deps, env, msg),
        QueryMsg::CanExecute { sender, message } => can_execute(deps, env, sender, message),
    }
}

fn can_execute(
    deps: Deps,
    env: Env,
    sender: String,
    _msg: UniversalMsg,
) -> Result<Binary, ContractError> {
    let valid_sender = deps.api.addr_validate(&sender)?;
    let session_key = SESSIONKEYS.load(deps.storage, &valid_sender)?;
    ensure!(
        !session_key.is_expired(env.block.time),
        ContractError::Std(StdError::GenericErr {
            msg: "Session key is expired".to_string(),
        })
    );
    Ok(to_binary(&CanExecuteResponse {
        can_execute: session_key.admin_permissions,
    })?)
}

fn handle_andr_hook(_deps: Deps, msg: AndromedaHook) -> Result<Binary, ContractError> {
    match msg {
        AndromedaHook::OnExecute { sender: _, .. } => {
            /* let is_included = includes_address(deps.storage, &sender)?;
            let is_inclusive = IS_INCLUSIVE.load(deps.storage)?;
            if is_included != is_inclusive {
                Err(ContractError::Unauthorized {})
            } else { */
            Ok(to_binary(&None::<Response>)?)
            // }
        }
        _ => Ok(to_binary(&None::<Response>)?),
    }
}

fn handle_andromeda_query(
    deps: Deps,
    env: Env,
    msg: AndromedaQuery,
) -> Result<Binary, ContractError> {
    match msg {
        AndromedaQuery::Get(_data) => {
            /*let address: String = parse_message(&data)?;
            encode_binary(&query_address(deps, &address)?)*/
            encode_binary(&cosmwasm_std::Empty {})
        }
        _ => ADOContract::default().query(deps, env, msg, query),
    }
}
