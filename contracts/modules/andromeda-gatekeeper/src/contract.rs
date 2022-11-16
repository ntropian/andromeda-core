use andromeda_modules::gatekeeper::{
    Authorization, AuthorizationsResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
    UniversalMsg,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, to_binary, Addr, Api, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdError, StdResult, Uint128,
};
use cosmwasm_std::{from_binary, Empty, WasmMsg};
use cw2::{get_contract_version, set_contract_version};

use crate::state::{COUNTER, OWNER, is_owner};
use crate::{error::ContractError, state::authorizations};
use ado_base::ADOContract;
use common::{
    ado_base::{hooks::AndromedaHook, AndromedaQuery, InstantiateMsg as BaseInstantiateMsg},
    encode_binary, parse_message,
};
use cw_storage_plus::Bound;
use semver::Version;
use serde_json_value_wasm::{Value, Map};

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
    COUNTER.save(deps.storage, &Uint128::from(0u128))?;
    OWNER.save(deps.storage, &deps.api.addr_validate(&msg.owner)?)?;
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
        .map_err(|e| ContractError::CommonError(e))
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
    }
}

pub fn add_authorization(
    deps: DepsMut,
    info: MessageInfo,
    authorization: Authorization,
) -> Result<Response, ContractError> {
    if !ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())? {
        if !is_owner(deps.storage, info.sender.to_string())? {
            return Err(ContractError::Unauthorized {});
        }
    }
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
            if key.authorizations.len() == 0 {
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
    if authorization.identifier > 0 {
        let auth = authorizations()
            .idx
            .identifier
            .item(deps.storage, authorization.identifier)?;
        return match auth {
            None => Err(ContractError::NoSuchAuthorization {}),
            Some(res) => Ok(AuthorizationsResponse { authorizations: vec![res] }),
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
                                                .map_err(|e| ContractError::Std(e))?});
                                        }
                                        Some(_) =>  working_auths = authorizations()
                                        .range(deps.storage, None, None, Order::Ascending)
                                        .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?,
                                    }
                                }
                                // some wasmaction_name
                                Some(name) => working_auths = authorizations()
                                .idx
                                .wasmaction_name
                                .prefix(name)
                                .range(deps.storage, None, None, Order::Ascending)
                                .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?,
                            }
                        }
                        // some message_name
                        Some(name) => working_auths = authorizations()
                        .idx
                        .message_name
                        .prefix(name)
                        .range(deps.storage, None, None, Order::Ascending)
                        .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?,
                    }
                }
                // some contract
                Some(addy) => working_auths = authorizations()
                    .idx
                    .contract
                    .prefix(addy)
                    .range(deps.storage, None, None, Order::Ascending)
                    .collect::<StdResult<Vec<(Vec<u8>, Authorization)>>>()?,
            }
        },
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
        process_auths(&mut working_auths, |auth| auth.contract == Some(addy.clone()));
    }
    if let Some(name) = authorization.message_name {
        process_auths(&mut working_auths, |auth| auth.message_name == Some(name.clone()));
    }
    if let Some(name) = authorization.wasmaction_name {
        process_auths(&mut working_auths, |auth| auth.wasmaction_name == Some(name.clone()));
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
        working_auths.retain(|item| item.1.fields != None );

        // if anything remains, iterate through
        if working_auths.len() < 1 {
            return Err(ContractError::NoSuchAuthorization {});
        } else {
            check_authorizations_against_fields(&mut working_auths, &msg_obj);
        }
    }

    // If we're using `msg` param to match on fields, then `None` is a valid
    // match as it means "any set of fields is OK"
    if let Some(msg) = msg {
        // in this case, Nones are a go ahead!
        let mut none_auths = working_auths.clone();
        none_auths.retain(|item| item.1.fields != None );
        if none_auths.len() > 0 {
            // just return here as we have authorization(s) that don't require fields
            // note that this means the a caller is not guaranteed a complete list of
            // applicable authorizations: returning an authorization may have other utilities,
            // but to find all matching authorizations, `authorization` param should be used
            // instead of `msg`
            return Ok(AuthorizationsResponse { authorizations: none_auths })
        }
        let msg_value: Value = serde_json_wasm::from_slice(&msg)?;
        let msg_obj: &serde_json_value_wasm::Map<String, Value> = match msg_value.as_object() {
            Some(obj) => obj,
            None => return Err(ContractError::Unauthorized {}),
        };
        check_authorizations_against_fields(&mut working_auths, msg_obj);
    }
    Ok(AuthorizationsResponse { authorizations: working_auths })
}

pub fn process_auths(
    auths: &mut Vec<(Vec<u8>, Authorization)>,
    predicate: impl Fn(Authorization) -> bool,
) {
    auths.retain(|item| predicate(item.1.clone()));
}

pub fn check_authorizations_against_fields(working_auths: &mut Vec<(Vec<u8>,Authorization)>, msg_obj: &serde_json_value_wasm::Map<String, Value>) -> Result<(), ContractError> {
    #[allow(unused_assignments)]
    for mut auth_count in 0..working_auths.len() {
        // we're editing working_auths in place, not iterating,
        // so make sure we're not at the end...
        if auth_count == working_auths.len() {
            break;
        }
        let this_idx = working_auths[auth_count].0.clone();
        let this_auth: Authorization = working_auths[auth_count].1.clone();
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
                            auth_count = auth_count.saturating_sub(1);
                            break 'inner;
                        }
                        // else, keep this auth
                    } else {
                        // remove this auth from results, since it doesn't include the required field
                        working_auths.retain(|item| item.0 != this_idx);
                        auth_count = auth_count.saturating_sub(1);
                        break 'inner;
                    }
                }
            }
            None => {
                // unreachable in runtime as Nones have been stripped
                panic!("None encountered when no Nones are expected");
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
    if !ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())? {
        if !is_owner(deps.storage, info.sender.to_string())? {
            return Err(ContractError::Unauthorized {});
        }
    }
    let found_auth_key = match get_authorizations_with_idx(deps.as_ref(), authorization, None) {
        Err(_) => return Err(ContractError::NoSuchAuthorization {}),
        Ok(key) => key,
    };
    match found_auth_key.authorizations.len() {
        0 => Err(ContractError::NoSuchAuthorization {}),
        1 => {
            authorizations().remove(deps.storage, &found_auth_key.authorizations[0].0)?;
            Ok(Response::default())
        },
        _ => Err(ContractError::MultipleMatchingAuthorizations {vector: found_auth_key.authorizations}),
    }
}

pub fn rm_all_matching_authorizations(
    deps: DepsMut,
    info: MessageInfo,
    authorization: Authorization,
) -> Result<Response, ContractError> {
    if !ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())? {
        if !is_owner(deps.storage, info.sender.to_string())? {
            return Err(ContractError::Unauthorized {});
        }
    }
    let found_auth_key = match get_authorizations_with_idx(deps.as_ref(), authorization, None) {
        Err(_) => return Err(ContractError::NoSuchAuthorization {}),
        Ok(key) => key,
    };
    for key in found_auth_key.authorizations{
        authorizations().remove(deps.storage, &key.0)?;
    }
    Ok(Response::default())
}

pub fn check_msgs(
    deps: Deps,
    sender: Addr,
    msgs: Vec<UniversalMsg>,
) -> Result<bool, ContractError> {
    for msg in msgs.into_iter() {
        if !check_msg(deps, sender.clone(), msg)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn check_msg(deps: Deps, sender: Addr, msg: UniversalMsg) -> Result<bool, ContractError> {
    match msg {
        UniversalMsg::Andromeda(msg) => {
            todo!()
        }
        UniversalMsg::Legacy(msg) => {
            match msg {
                cosmwasm_std::CosmosMsg::Bank(_) => todo!(),
                cosmwasm_std::CosmosMsg::Custom(_) => todo!(),
                cosmwasm_std::CosmosMsg::Staking(_) => todo!(),
                cosmwasm_std::CosmosMsg::Distribution(_) => todo!(),
                cosmwasm_std::CosmosMsg::Stargate { type_url, value } => todo!(),
                cosmwasm_std::CosmosMsg::Ibc(_) => todo!(),
                cosmwasm_std::CosmosMsg::Wasm(msg) => {
                    match msg {
                        WasmMsg::Execute {
                            contract_addr,
                            msg,
                            funds,
                        } => {
                            check_wasm_msg(
                                deps,
                                deps.api.addr_validate(&contract_addr)?,
                                sender,
                                msg,
                            );
                        }
                        WasmMsg::Instantiate {
                            admin,
                            code_id,
                            msg,
                            funds,
                            label,
                        } => todo!(),
                        WasmMsg::Migrate {
                            contract_addr,
                            new_code_id,
                            msg,
                        } => todo!(),
                        WasmMsg::UpdateAdmin {
                            contract_addr,
                            admin,
                        } => todo!(),
                        WasmMsg::ClearAdmin { contract_addr } => todo!(),
                        _ => todo!(),
                    };
                }
                cosmwasm_std::CosmosMsg::Gov(_) => todo!(),
                _ => todo!(),
            };
        }
    }
    Ok(true)
}

/// Checks a WasmMsg::Execute (MsgExecuteContract) against authorizations table.
/// Returns true if matching authorization found. If not found, returns an error
/// rather than false, since the error type provides additional information that
/// may be useful to the caller.
pub fn check_wasm_msg(
    deps: Deps,
    target_contract: Addr,
    sender: Addr,
    msg: Binary,
) -> Result<bool, ContractError> {
    let wasm_msg: WasmMsg = from_binary(&msg)?;
    // check there is an authorization for this contract


    
    Ok(true)
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
        None => {
            authorizations()
                .range(deps.storage, start_raw, None, Order::Ascending)
                .take(limit)
                .map(|item| item.unwrap() )
                .collect::<Vec<(Vec<u8>, Authorization)>>()
        }
        Some(target) => {
            authorizations()
                .idx
                .contract
                .prefix(deps.api.addr_validate(&target)?)
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .map(|item| item.unwrap() )
                .collect::<Vec<(Vec<u8>, Authorization)>>()
        }
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
                    identifier: identifier.unwrap_or_else(|| 0u16),
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
        QueryMsg::CheckTransaction { msgs, sender } => {
            to_binary(&check_msgs(deps, deps.api.addr_validate(&sender)?, msgs).map_err(|e| {
                common::error::ContractError::Std(StdError::GenericErr { msg: e.to_string() })
            })?)
            .map_err(|e| {
                common::error::ContractError::Std(StdError::GenericErr { msg: e.to_string() })
            })
        }
    }
}

fn handle_andr_hook(
    deps: Deps,
    msg: AndromedaHook,
) -> Result<Binary, common::error::ContractError> {
    match msg {
        AndromedaHook::OnExecute { sender, .. } => {
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
            let authorization: Authorization = parse_message(&data)?;
            encode_binary(&Empty {})
        }
        _ => ADOContract::default().query(deps, env, msg, query),
    }
}

#[cfg(test)]
mod tests {
    use andromeda_modules::gatekeeper::{TestExecuteMsg, TestFieldsExecuteMsg};

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Api, CosmosMsg};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            owner: "owner".to_string(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn add_authorization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let query_msg = QueryMsg::Authorizations {
            identifier: Some(0u16),
            actor: None,
            fields: None,
            message_name: None,
            wasmaction_name: None,
            target_contract: Some("targetcontract".to_string()),
            limit: None,
            start_after: None,
        };

        let msg = InstantiateMsg {
            owner: "owner".to_string(),
        };
        let info = mock_info("user", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // non-operator cannot add authorization
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("anyone").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: None,
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

        // zero authorizations
        let raw_res = query(deps.as_ref(), mock_env(), query_msg.clone());
        let res: AuthorizationsResponse =
            from_binary(&raw_res.unwrap()).unwrap();
        assert_eq!(res.authorizations.len(), 0);

        // operator can add authorization
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: None,
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now one authorization
        let res: AuthorizationsResponse =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
        assert_eq!(res.authorizations.len(), 1);
        println!("res: {:?}", res);

        // given action should fail if NOT BY ACTOR
        let msg = QueryMsg::CheckTransaction {
            sender: "anyone".to_string(),
            msgs: vec![UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute{
                msg: to_binary(&TestExecuteMsg { 
                foo: "bar".to_string()
                }).unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            }))],
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // given action should fail if WRONG TARGET CONTRACT
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msgs: vec![UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute{
                msg: to_binary(&TestExecuteMsg { 
                foo: "bar".to_string()
                }).unwrap(),
                contract_addr: "badcontract".to_string(),
                funds: vec![],
            }))],
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // given action should fail if wrong actor
        let msg = QueryMsg::CheckTransaction {
            sender: "badactor".to_string(),
            msgs: vec![UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute{
                msg: to_binary(&TestExecuteMsg { 
                foo: "bar".to_string()
                }).unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            }))],
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // given action should succeed if contract correct (no field checking yet)
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msgs: vec![UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute{
                msg: to_binary(&TestExecuteMsg { 
                foo: "bar".to_string()
                }).unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            }))],
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // unauthorized user cannot remove an authorization
        let info = mock_info("baduser", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: None,
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();

        // let's remove an authorization successfully now
        let info = mock_info("owner", &coins(2, "token"));
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now zero authorizations
        let res: AuthorizationsResponse =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(res.authorizations.len(), 0);
        println!("res: {:?}", res);

        //and action fails where before it succeeded
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msgs: vec![UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute{
                msg: to_binary(&TestExecuteMsg { 
                foo: "bar".to_string()
                }).unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            }))],
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();
    }

    #[test]
    fn authorization_fields() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let query_msg = QueryMsg::Authorizations {
            identifier: None,
            wasmaction_name: None,
            message_name: None,
            fields: None,
            actor: None,
            target_contract: Some("targetcontract".to_string()),
            limit: None,
            start_after: None,
        };

        let msg = InstantiateMsg {
            owner: "owner".to_string(),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // add authorization with fields
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_fields_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // given action should succeed if contract correct
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msgs: vec![UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute{
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "engage".to_string(),
                }).unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            }))],
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // let's remove but with wrong fields specified... should FAIL
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                identifier: 0u16,
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_fields_execute_msg".to_string()),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("tactic".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        
        // still one authorization
        let res: AuthorizationsResponse =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
        assert_eq!(res.authorizations.len(), 1);
        println!("res: {:?}", res);

        // let's remove the authorization with no field checking... should SUCCEED
        // tbd: maybe we want this to fail
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                identifier: 0u16,
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_fields_execute_msg".to_string()),
                fields: None,
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now zero authorizations
        let res: AuthorizationsResponse =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
        assert_eq!(res.authorizations.len(), 0);
        println!("res: {:?}", res);

        // let's test with just strategy, and no qualification on recipient
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_fields_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: Some([("strategy".to_string(), "engage".to_string())].to_vec()),
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // fails if strategy is wrong
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msgs: vec![UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute{
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "assimmilate".to_string(),
                }).unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            }))],
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // succeeds if strategy is allowed
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msgs: vec![UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute{
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "engage".to_string(),
                }).unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            }))],
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // remove succeeds even with more fields specified (denying a more specific auth than exists)
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_fields_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // now removal fails as no longer exists
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_fields_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: Some(
                    [
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    }

    #[test]
    fn handling_repeat_authorization_fields() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            owner: "owner".to_string(),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // add authorization with fields
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_fields_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // adding the same again should cause an error
        // in the future, maybe change this test to update expiration instead
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("test_fields_execute_msg".to_string()),
                wasmaction_name: Some("MsgExecuteContract".to_string()),
                fields: Some(
                    [
                        ("recipient".to_string(), "picard".to_string()),
                        ("strategy".to_string(), "engage".to_string()),
                    ]
                    .to_vec(),
                ),
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    }
}
