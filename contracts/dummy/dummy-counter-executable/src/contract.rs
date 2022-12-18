#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{CheaterDetectedResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::CHEATER_DETECTED;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:dummy-counter-executable";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    CHEATER_DETECTED.save(deps.storage, &false)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::KobayashiMaru { captain, strategy } => {
            if strategy == *"cheat" {
                CHEATER_DETECTED.save(deps.storage, &true)?;
            }
            let response = Response::new()
                .add_attribute("captain", captain)
                .add_attribute("strategy", strategy);
            Ok(response)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::CheaterDetected {} => to_binary(&detect_cheater(deps)?),
    }
}

fn detect_cheater(deps: Deps) -> StdResult<CheaterDetectedResponse> {
    Ok(CheaterDetectedResponse {
        cheater_detected: CHEATER_DETECTED.load(deps.storage)?,
    })
}

#[cfg(test)]
mod tests {}
