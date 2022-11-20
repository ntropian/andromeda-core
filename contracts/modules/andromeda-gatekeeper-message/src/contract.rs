use andromeda_modules::gatekeeper_common::{
    is_legacy_owner, update_legacy_owner, InstantiateMsg, UniversalMsg, LEGACY_OWNER,
};
use andromeda_modules::gatekeeper_message::{
    Authorization, AuthorizationsResponse, ExecuteMsg, MigrateMsg, QueryMsg,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, to_binary, Addr, Api, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdError, StdResult, Uint128,
};
use cosmwasm_std::{Empty, WasmMsg};
use cw2::{get_contract_version, set_contract_version};

use crate::state::COUNTER;
use crate::{error::ContractError, state::authorizations};
use ado_base::ADOContract;
use common::{
    ado_base::{hooks::AndromedaHook, AndromedaQuery, InstantiateMsg as BaseInstantiateMsg},
    encode_binary, parse_message,
};
use cw_storage_plus::Bound;
use semver::Version;
use serde_json_value_wasm::{Map, Value};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-gatekeeper";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 30;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    LEGACY_OWNER.save(deps.storage, &msg.legacy_owner)?;
    COUNTER.save(deps.storage, &Uint128::from(0u128))?;
    ADOContract::default()
        .instantiate(
            deps.storage,
            env,
            deps.api,
            info,
            BaseInstantiateMsg {
                ado_type: "gatekeeper".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),
                operators: None,
                modules: None,
                primitive_contract: None,
            },
        )
        .map_err(ContractError::CommonError)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, common::error::ContractError> {
    match msg {
        ExecuteMsg::AddAuthorization { new_authorization } => {
            add_authorization(deps, info, new_authorization).map_err(|e| {
                common::error::ContractError::Std(StdError::GenericErr { msg: e.to_string() })
            })
        }
        ExecuteMsg::RemoveAuthorization {
            authorization_to_remove,
        } => rm_authorization(deps, info, authorization_to_remove).map_err(|e| {
            common::error::ContractError::Std(StdError::GenericErr { msg: e.to_string() })
        }),
        ExecuteMsg::RmAllMatchingAuthorizations {
            authorization_to_remove,
        } => rm_all_matching_authorizations(deps, info, authorization_to_remove).map_err(|e| {
            common::error::ContractError::Std(StdError::GenericErr { msg: e.to_string() })
        }),
        ExecuteMsg::AndrReceive(msg) => {
            ADOContract::default().execute(deps, env, info, msg, execute)
        }
        ExecuteMsg::UpdateLegacyOwner { new_owner } => {
            let valid_new_owner = deps.api.addr_validate(&new_owner)?;
            update_legacy_owner(deps, info, valid_new_owner)
        }
    }
}

pub fn add_authorization(
    deps: DepsMut,
    info: MessageInfo,
    authorization: Authorization,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.as_ref().storage, info.sender.as_str())?
            || is_legacy_owner(deps.as_ref(), info.sender)?,
        ContractError::Unauthorized {}
    );
    match get_authorizations_with_idx(deps.as_ref(), authorization.clone(), None) {
        Err(_) => {
            let auth_count = COUNTER.load(deps.as_ref().storage)?.to_owned();
            authorizations().save(
                deps.storage,
                auth_count.u128().to_string().as_bytes(),
                &authorization,
            )?;
        }
        Ok(key) => {
            if key.authorizations.is_empty() {
                let auth_count = COUNTER.load(deps.as_ref().storage)?.to_owned();
                authorizations().save(
                    deps.storage,
                    auth_count.u128().to_string().as_bytes(),
                    &authorization,
                )?;
            } else {
                // may add expiration here instead in future version
                return Err(ContractError::CustomError {
                    val: "temporary error: auth exists".to_string(),
                });
            }
        }
    }
    Ok(Response::default())
}

/// `authorization` may have:
///
/// `identifier` - A u16 pinpointing a specific authorization. If 0, ignore.
/// `actor` - an Option<Addr>. If Some, find only authorizations which authorize
/// this actor.
///
/// `contract` - an Option<Addr>. If Some, find only authorizations which allow
/// actor(s) to take actions on this contract.
///
/// `message_name` - Option<String>. If Some, find only authorizations matching
/// this message name (for example, "MsgExecuteContract"). Note that a universal
/// authorization by contract or wasmaction_name will result in `true` even if
/// a specific `message_name` is not included.
///
/// `wasmaction_name` – Option<String>. Applicable only to MsgExecuteContract,
/// but works if `message_name` is None. If Some, find only authorizations that
/// are wasm execute messages with this action name (for example, "transfer").
/// Note that a universal authorization by contract or message_name will result in
/// `true` even if a specific `wasmaction_name` is not included.
///
/// `fields` – Option<Vec<(String, String)>. Finds only authorizations which
/// allow messages with certain parameters. For example, if checking whether
/// messages with `token_id` set to `15` are allowed, `fields` should be:
/// vec![("token_id", "15")]. Note that not finding such an authorization does not
/// mean a related transaction will not succeed: the sender may have, for example,
/// a universal authorization on the particular contract or message_name.
fn get_authorizations_with_idx(
    deps: Deps,
    authorization: Authorization,
    msg: Option<Binary>,
) -> Result<AuthorizationsResponse, ContractError> {
    // if identifier specified, our task is easy
    if authorization.identifier > 0u16 {
        let auth = authorizations()
            .idx
            .identifier
            .item(deps.storage, authorization.identifier)?;
        return match auth {
            None => Err(ContractError::NoSuchAuthorization {
                loc: "get_authorizations_with_idx_1".to_string(),
            }),
            Some(res) => Ok(AuthorizationsResponse {
                authorizations: vec![res],
            }),
        };
    }

    // Otherwise, grab authorizations and filter down.
    // First, we need a base. The order here is an educated guess
    // on which filters will be used more often.
    let mut working_auths: Vec<(Vec<u8>, Authorization)>;
    let match_auth = authorization.clone();
    match match_auth.actor {
        None => {
            match match_auth.contract {
                None => {
                    match match_auth.message_name {
                        None => {
                            match match_auth.wasmaction_name {
                                None => {
                                    match match_auth.fields {
                                        None => {
                                            // return all authorizations!
                                            return Ok(AuthorizationsResponse { authorizations: authorizations().range(deps.storage, None, None, Order::Ascending).collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()
                                                .map_err(ContractError::Std)?});
                                        }
                                        // some fields, but we can't use .idx for those (field checking is later)
                                        Some(_) => working_auths = authorizations()
                                            .range(deps.storage, None, None, Order::Ascending)
                                            .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>(
                                            )?,
                                    }
                                }
                                // some wasmaction_name
                                Some(name) => {
                                    working_auths = authorizations()
                                        .idx
                                        .wasmaction_name
                                        .prefix(name)
                                        .range(deps.storage, None, None, Order::Ascending)
                                        .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?
                                }
                            }
                        }
                        // some message_name
                        Some(name) => {
                            working_auths = authorizations()
                                .idx
                                .message_name
                                .prefix(name)
                                .range(deps.storage, None, None, Order::Ascending)
                                .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?
                        }
                    }
                }
                // some contract
                Some(addy) => {
                    working_auths = authorizations()
                        .idx
                        .contract
                        .prefix(addy)
                        .range(deps.storage, None, None, Order::Ascending)
                        .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?
                }
            }
        }
        // some actor
        Some(addy) => {
            working_auths = authorizations()
                .idx
                .actor
                .prefix(addy)
                .range(deps.storage, None, None, Order::Ascending)
                .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?;
        }
    }

    // Now we can filter down our base
    if let Some(addy) = authorization.contract {
        process_auths(&mut working_auths, |auth| {
            auth.contract == Some(addy.clone())
        });
    }
    if let Some(name) = authorization.message_name {
        process_auths(&mut working_auths, |auth| {
            auth.message_name == Some(name.clone())
        });
    }
    if let Some(name) = authorization.wasmaction_name {
        process_auths(&mut working_auths, |auth| {
            auth.wasmaction_name == Some(name.clone())
        });
    }

    // The final filter, by fields, is most complex.

    // If we're using `authorization` param to look up and are specifying fields,
    // we want a field match. Authorizations with `fields` as `None`
    // won't be returned as specific authorization is being sought.
    if let Some(vals) = authorization.fields {
        let msg_obj: Map<String, Value> = vals
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect();

        // first let's strip any Nones
        working_auths.retain(|item| item.1.fields != None);

        // if anything remains, iterate through
        if working_auths.is_empty() {
            return Err(ContractError::NoSuchAuthorization {
                loc: "get_authorizations_with_idx_2".to_string(),
            });
        } else {
            check_authorizations_against_fields(&mut working_auths, &msg_obj)?;
        }
    }

    // If we're using `msg` param to match on fields, then `None` is a valid
    // match as it means "any set of fields is OK"
    if let Some(msg) = msg {
        // in this case, Nones are a go ahead!
        let mut none_auths = working_auths.clone();
        none_auths.retain(|item| item.1.fields == None);
        if !none_auths.is_empty() {
            // just return here as we have authorization(s) that don't require fields
            // note that this means the a caller is not guaranteed a complete list of
            // applicable authorizations: returning an authorization may have other utilities,
            // but to find all matching authorizations, `authorization` param should be used
            // instead of `msg`
            return Ok(AuthorizationsResponse {
                authorizations: none_auths,
            });
        }
        let msg_value: Value = serde_json_wasm::from_slice(&msg)?;
        let msg_obj: &serde_json_value_wasm::Map<String, Value> = match msg_value.as_object() {
            Some(obj) => {
                if obj.keys().len() > 1 {
                    return Err(ContractError::TooManyMessages {});
                } else {
                    // unsafe unwraps for now
                    let key = obj.keys().next().unwrap();
                    obj[key].as_object().unwrap()
                }
            }
            None => return Err(ContractError::Unauthorized {}),
        };
        check_authorizations_against_fields(&mut working_auths, msg_obj)?;
    }
    Ok(AuthorizationsResponse {
        authorizations: working_auths,
    })
}

pub fn process_auths(
    auths: &mut Vec<(Vec<u8>, Authorization)>,
    predicate: impl Fn(Authorization) -> bool,
) {
    auths.retain(|item| predicate(item.1.clone()));
}

pub fn check_authorizations_against_fields(
    working_auths: &mut Vec<(Vec<u8>, Authorization)>,
    msg_obj: &serde_json_value_wasm::Map<String, Value>,
) -> Result<(), ContractError> {
    let mut offset = 0usize;
    for auth_count in 0..working_auths.len() {
        // we're editing working_auths in place, not iterating,
        // so make sure we're not at the end...
        if auth_count - offset == working_auths.len() {
            break;
        }
        let this_idx = working_auths[auth_count - offset].0.clone();
        let this_auth: Authorization = working_auths[auth_count - offset].1.clone();
        match this_auth.fields {
            Some(vals) => {
                'inner: for kv in 0..vals.len() {
                    let this_key: String = vals[kv].clone().0;
                    let this_value: String = vals[kv].clone().1;
                    if msg_obj.contains_key(&this_key) {
                        if msg_obj[&this_key] != this_value && kv == vals.len() - 1 {
                            // remove this auth from results, since its field mismatches
                            // still to be implemented: range and != matching logic
                            working_auths.retain(|item| item.0 != this_idx);
                            offset = offset + 1;
                            break 'inner;
                        }
                        // else, keep this auth
                    } else {
                        // remove this auth from results, since it doesn't include the required field
                        working_auths.retain(|item| item.0 != this_idx);
                        offset = offset + 1;
                        break 'inner;
                    }
                }
            }
            None => {
                // unreachable in runtime as Nones have been stripped
                return Err(ContractError::CustomError {
                    val: "None encountered when no Nones are expected".to_string(),
                });
                // working_auths.retain(|item| item.0 != working_auths[auth_count].0);
            }
        }
    }
    Ok(())
}

pub fn rm_authorization(
    deps: DepsMut,
    info: MessageInfo,
    authorization: Authorization,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.as_ref().storage, info.sender.as_str())?
            || is_legacy_owner(deps.as_ref(), info.sender)?,
        ContractError::Unauthorized {}
    );
    let found_auth_key = match get_authorizations_with_idx(deps.as_ref(), authorization, None) {
        Err(_) => {
            return Err(ContractError::NoSuchAuthorization {
                loc: "rm_authorization_1".to_string(),
            })
        }
        Ok(key) => key,
    };
    match found_auth_key.authorizations.len() {
        0 => Err(ContractError::NoSuchAuthorization {
            loc: "rm_authorization_2".to_string(),
        }),
        1 => {
            authorizations().remove(deps.storage, &found_auth_key.authorizations[0].0)?;
            Ok(Response::default())
        }
        _ => Err(ContractError::MultipleMatchingAuthorizations {
            vector: found_auth_key.authorizations,
        }),
    }
}

pub fn rm_all_matching_authorizations(
    deps: DepsMut,
    info: MessageInfo,
    authorization: Authorization,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.as_ref().storage, info.sender.as_str())?
            || is_legacy_owner(deps.as_ref(), info.sender)?,
        ContractError::Unauthorized {}
    );
    let found_auth_key = match get_authorizations_with_idx(deps.as_ref(), authorization, None) {
        Err(e) => {
            return Err(e);
        }
        Ok(key) => key,
    };
    for key in found_auth_key.authorizations {
        authorizations().remove(deps.storage, &key.0)?;
    }
    Ok(Response::default())
}

#[allow(unused_variables)]
pub fn check_msg(
    deps: Deps,
    sender: Addr,
    msg: UniversalMsg,
) -> Result<AuthorizationsResponse, ContractError> {
    let todo_auths = AuthorizationsResponse {
        authorizations: vec![],
    };
    match msg {
        UniversalMsg::Andromeda(msg) => match msg {
            common::ado_base::AndromedaMsg::Receive(_) => Ok(todo_auths),
            common::ado_base::AndromedaMsg::UpdateOwner { address } => Ok(todo_auths),
            common::ado_base::AndromedaMsg::UpdateOperators { operators } => Ok(todo_auths),
            common::ado_base::AndromedaMsg::UpdateAppContract { address } => Ok(todo_auths),
            common::ado_base::AndromedaMsg::Withdraw {
                recipient,
                tokens_to_withdraw,
            } => Ok(todo_auths),
            common::ado_base::AndromedaMsg::RegisterModule { module } => Ok(todo_auths),
            common::ado_base::AndromedaMsg::DeregisterModule { module_idx } => Ok(todo_auths),
            common::ado_base::AndromedaMsg::AlterModule { module_idx, module } => Ok(todo_auths),
            common::ado_base::AndromedaMsg::RefreshAddress { contract } => Ok(todo_auths),
            common::ado_base::AndromedaMsg::RefreshAddresses { limit, start_after } => {
                Ok(todo_auths)
            }
        },
        UniversalMsg::Legacy(msg) => match msg {
            cosmwasm_std::CosmosMsg::Bank(_) => Ok(todo_auths),
            cosmwasm_std::CosmosMsg::Custom(_) => Ok(todo_auths),
            cosmwasm_std::CosmosMsg::Staking(_) => Ok(todo_auths),
            cosmwasm_std::CosmosMsg::Distribution(_) => Ok(todo_auths),
            cosmwasm_std::CosmosMsg::Stargate { type_url, value } => Ok(todo_auths),
            cosmwasm_std::CosmosMsg::Ibc(_) => Ok(todo_auths),
            cosmwasm_std::CosmosMsg::Wasm(msg) => match msg {
                WasmMsg::Execute {
                    contract_addr,
                    msg,
                    funds: _,
                } => check_wasm_msg(
                    deps,
                    Some(deps.api.addr_validate(&contract_addr)?),
                    sender,
                    msg,
                    "MsgExecuteContract".to_string(),
                ),
                WasmMsg::Instantiate {
                    admin: _,
                    code_id: _,
                    msg,
                    funds: _,
                    label: _,
                } => check_wasm_msg(
                    deps,
                    None,
                    sender,
                    msg,
                    "MsgInstantiateContract".to_string(),
                ),
                WasmMsg::Migrate {
                    contract_addr,
                    new_code_id,
                    msg,
                } => Ok(todo_auths),
                WasmMsg::UpdateAdmin {
                    contract_addr,
                    admin,
                } => Ok(todo_auths),
                WasmMsg::ClearAdmin { contract_addr } => Ok(todo_auths),
                _ => Ok(todo_auths),
            },
            cosmwasm_std::CosmosMsg::Gov(_) => Ok(todo_auths),
            _ => Ok(todo_auths),
        },
    }
}

/// Checks a WasmMsg::Execute (MsgExecuteContract) against authorizations table.
/// Returns any matching authorizations.
pub fn check_wasm_msg(
    deps: Deps,
    target_contract: Option<Addr>,
    sender: Addr,
    msg: Binary,
    message_name: String,
) -> Result<AuthorizationsResponse, ContractError> {
    let msg_value: Value = serde_json_wasm::from_slice(&msg)?;
    let msg_obj: &serde_json_value_wasm::Map<String, Value> = match msg_value.as_object() {
        Some(obj) => obj,
        None => return Err(ContractError::Unauthorized {}),
    };
    let wasmaction_name = Some(match msg_obj.keys().next() {
        Some(key) => key.to_string(),
        None => {
            return Err(ContractError::CustomError {
                val: "No execute message contents".to_string(),
            })
        }
    });
    let auths = get_authorizations_with_idx(
        deps,
        Authorization {
            identifier: 0u16,
            actor: Some(sender),
            contract: target_contract,
            wasmaction_name,
            message_name: Some(message_name),
            fields: None,
        },
        Some(msg),
    )?;
    Ok(auths)
}

pub fn maybe_addr(api: &dyn Api, human: Option<String>) -> StdResult<Option<Addr>> {
    human.map(|x| api.addr_validate(&x)).transpose()
}

pub fn query_authorizations(
    deps: Deps,
    target_contract: Option<String>,
    limit: Option<u32>,
    start_after: Option<String>,
) -> Result<AuthorizationsResponse, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start_raw = start_after
        .clone()
        .map(|s| Bound::ExclusiveRaw(s.into_bytes()));
    let start_addr = maybe_addr(deps.api, start_after)?;
    let start = start_addr.map(|addr| Bound::exclusive(addr.as_ref()));
    let authorizations = match target_contract {
        None => authorizations()
            .range(deps.storage, start_raw, None, Order::Ascending)
            .take(limit)
            .map(|item| item.unwrap())
            .collect::<Vec<(Vec<u8>, Authorization)>>(),
        Some(target) => authorizations()
            .idx
            .contract
            .prefix(deps.api.addr_validate(&target)?)
            .range(deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.unwrap())
            .collect::<Vec<(Vec<u8>, Authorization)>>(),
    };
    Ok(AuthorizationsResponse { authorizations })
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
        ContractError::CommonError(common::error::ContractError::CannotMigrate {
            previous_contract: stored.contract,
        })
    );

    // New version has to be newer/greater than the old version
    ensure!(
        storage_version < version,
        ContractError::CommonError(common::error::ContractError::CannotMigrate {
            previous_contract: stored.version,
        })
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
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, common::error::ContractError> {
    match msg {
        QueryMsg::AndrHook(msg) => handle_andr_hook(deps, msg),
        QueryMsg::AndrQuery(msg) => handle_andromeda_query(deps, env, msg),
        #[allow(unused_variables)]
        QueryMsg::Authorizations {
            identifier,
            actor,
            target_contract,
            message_name,
            wasmaction_name,
            fields,
            // pagination not implemented yet
            limit,
            start_after,
        } => Ok(to_binary(
            &get_authorizations_with_idx(
                deps,
                Authorization {
                    identifier: identifier.unwrap_or(0u16),
                    actor: actor.map(|inner| deps.api.addr_validate(&inner).unwrap()),
                    contract: target_contract.map(|ct| deps.api.addr_validate(&ct).unwrap()),
                    message_name,
                    wasmaction_name,
                    fields,
                },
                None,
            )
            .map_err(|e| {
                common::error::ContractError::Std(StdError::GenericErr { msg: e.to_string() })
            })?,
        )?),
        QueryMsg::CheckTransaction { msg, sender } => to_binary(
            &check_msg(deps, deps.api.addr_validate(&sender)?, msg).map_err(|e| {
                common::error::ContractError::Std(StdError::GenericErr { msg: e.to_string() })
            })?,
        )
        .map_err(|e| {
            common::error::ContractError::Std(StdError::GenericErr { msg: e.to_string() })
        }),
    }
}

fn handle_andr_hook(
    _deps: Deps,
    msg: AndromedaHook,
) -> Result<Binary, common::error::ContractError> {
    match msg {
        AndromedaHook::OnExecute { sender: _, .. } => {
            Ok(to_binary(&Empty {})?) //todo
        }
        _ => Ok(to_binary(&None::<Response>)?),
    }
}

fn handle_andromeda_query(
    deps: Deps,
    env: Env,
    msg: AndromedaQuery,
) -> Result<Binary, common::error::ContractError> {
    match msg {
        AndromedaQuery::Get(data) => {
            let _authorization: Authorization = parse_message(&data)?;
            encode_binary(&Empty {})
        }
        _ => ADOContract::default().query(deps, env, msg, query),
    }
}
