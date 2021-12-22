use cosmwasm_std::{attr, Deps, DepsMut, MessageInfo, Order, Response, StdResult};

use crate::error::ContractError;
use crate::ownership::is_contract_owner;
use crate::require;
use cw_storage_plus::Map;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const OPERATORS: Map<String, bool> = Map::new("operators");

pub fn execute_update_operators(
    deps: DepsMut,
    info: MessageInfo,
    operators: Vec<String>,
) -> Result<Response, ContractError> {
    require(
        is_contract_owner(deps.storage, info.sender.to_string())?,
        ContractError::Unauthorized {},
    )?;

    let keys: Vec<Vec<u8>> = OPERATORS
        .keys(deps.storage, None, None, Order::Ascending)
        .collect();
    for key in keys.iter() {
        OPERATORS.remove(deps.storage, String::from_utf8(key.clone())?);
    }

    for x in operators.iter() {
        OPERATORS.save(deps.storage, x.clone(), &true)?;
    }

    Ok(Response::new().add_attributes(vec![attr("action", "update_operators")]))
}

pub fn query_is_operator(deps: Deps, addr: String) -> StdResult<IsOperatorResponse> {
    let operator = OPERATORS.may_load(deps.storage, addr)?;
    Ok(IsOperatorResponse {
        is_operator: operator.is_some(),
    })
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct IsOperatorResponse {
    pub is_operator: bool,
}

#[cfg(test)]
mod tests {
    use crate::ownership::CONTRACT_OWNER;
    use cosmwasm_std::testing::{mock_dependencies, mock_info};
    use cosmwasm_std::Addr;

    use super::*;

    #[test]
    fn test_execute_update_operators() {
        let mut deps = mock_dependencies(&[]);
        let owner = String::from("owner");
        let owner_addr = Addr::unchecked(owner.clone());
        let operators = vec!["operator_000".to_string(), "operator_001".to_string()];

        let auth_info = mock_info(owner.as_str(), &[]);

        CONTRACT_OWNER
            .save(deps.as_mut().storage, &owner_addr)
            .unwrap();
        let unauth_info = mock_info("anyone", &[]);
        //check auth
        let resp =
            execute_update_operators(deps.as_mut(), unauth_info, operators.clone()).unwrap_err();
        let expected = ContractError::Unauthorized {};
        assert_eq!(resp, expected);

        let resp = execute_update_operators(deps.as_mut(), auth_info.clone(), operators).unwrap();
        let expected = Response::new().add_attributes(vec![attr("action", "update_operators")]);
        assert_eq!(resp, expected);
        //check
        let query_resp = query_is_operator(deps.as_ref(), "operator_001".to_string()).unwrap();
        assert!(query_resp.is_operator);

        //update another operators
        let _ = execute_update_operators(deps.as_mut(), auth_info, vec!["another".to_string()])
            .unwrap();
        //check to be removed operator_000, operator_001
        let query_resp = query_is_operator(deps.as_ref(), "operator_001".to_string()).unwrap();
        assert!(!query_resp.is_operator);
        let query_resp = query_is_operator(deps.as_ref(), "operator_000".to_string()).unwrap();
        assert!(!query_resp.is_operator);
    }
}
