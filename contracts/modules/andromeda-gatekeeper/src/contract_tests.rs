#[cfg(test)]
mod tests {
    use andromeda_modules::gatekeeper::{
        Authorization, AuthorizationsResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
        TestExecuteMsg, TestFieldsExecuteMsg, UniversalMsg,
    };
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, to_binary, Api, CosmosMsg, WasmMsg};

    use crate::contract::{execute, instantiate, query};

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
        let res: AuthorizationsResponse = from_binary(&raw_res.unwrap()).unwrap();
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
            msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            })),
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // given action should fail if WRONG TARGET CONTRACT
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                contract_addr: "badcontract".to_string(),
                funds: vec![],
            })),
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // given action should fail if wrong actor
        let msg = QueryMsg::CheckTransaction {
            sender: "badactor".to_string(),
            msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            })),
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // given action should succeed if contract correct (no field checking yet)
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            })),
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // unauthorized user cannot remove an authorization
        let info = mock_info("baduser", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("MsgExecuteContract".to_string()),
                wasmaction_name: Some("test_execute_msg".to_string()),
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
            msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: to_binary(&TestExecuteMsg {
                    foo: "bar".to_string(),
                })
                .unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            })),
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
                message_name: Some("MsgExecuteContract".to_string()),
                wasmaction_name: Some("test_fields_execute_msg".to_string()),
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
            msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "engage".to_string(),
                })
                .unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            })),
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // let's remove but with wrong fields specified... should FAIL
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                identifier: 0u16,
                wasmaction_name: Some("test_fields_execute_msg".to_string()),
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("MsgExecuteContract".to_string()),
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
                wasmaction_name: Some("test_fields_execute_msg".to_string()),
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("MsgExecuteContract".to_string()),
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
                message_name: Some("MsgExecuteContract".to_string()),
                wasmaction_name: Some("test_fields_execute_msg".to_string()),
                fields: Some([("strategy".to_string(), "engage".to_string())].to_vec()),
            },
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // fails if strategy is wrong
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "assimmilate".to_string(),
                })
                .unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            })),
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // succeeds if strategy is allowed
        let msg = QueryMsg::CheckTransaction {
            sender: "actor".to_string(),
            msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: to_binary(&TestFieldsExecuteMsg {
                    recipient: "picard".to_string(),
                    strategy: "engage".to_string(),
                })
                .unwrap(),
                contract_addr: "targetcontract".to_string(),
                funds: vec![],
            })),
        };
        let _res = query(deps.as_ref(), mock_env(), msg).unwrap();

        // remove succeeds even with more fields specified (denying a more specific auth than exists)
        let info = mock_info("owner", &coins(2, "token"));
        let msg = ExecuteMsg::RemoveAuthorization {
            authorization_to_remove: Authorization {
                identifier: 0u16,
                actor: Some(deps.api.addr_validate("actor").unwrap()),
                contract: Some(deps.api.addr_validate("targetcontract").unwrap()),
                message_name: Some("MsgExecuteContract".to_string()),
                wasmaction_name: Some("test_fields_execute_msg".to_string()),
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
                message_name: Some("MsgExecuteContract".to_string()),
                wasmaction_name: Some("test_fields_execute_msg".to_string()),
                fields: Some([("strategy".to_string(), "engage".to_string())].to_vec()),
            },
        };
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
                message_name: Some("MsgExecuteContract".to_string()),
                wasmaction_name: Some("test_fields_execute_msg".to_string()),
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
                message_name: Some("MsgExecuteContract".to_string()),
                wasmaction_name: Some("test_fields_execute_msg".to_string()),
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
