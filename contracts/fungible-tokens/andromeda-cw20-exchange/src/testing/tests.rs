use andromeda_fungible_tokens::cw20_exchange::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, Sale};
use common::{app::AndrAddress, error::ContractError};
use cosmwasm_std::{
    testing::{mock_dependencies, mock_env, mock_info},
    to_binary, Addr, Uint128,
};
use cw20::Cw20ReceiveMsg;
use cw_asset::AssetInfo;

use crate::{
    contract::{execute, instantiate},
    state::{SALE, TOKEN_ADDRESS},
};

#[test]
pub fn test_instantiate() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
    let token_address = Addr::unchecked("cw20");

    instantiate(
        deps.as_mut(),
        env,
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let saved_token_address = TOKEN_ADDRESS.load(deps.as_ref().storage).unwrap();

    assert_eq!(saved_token_address.identifier, token_address.to_string())
}

#[test]
pub fn test_start_sale_invalid_token() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate: Uint128::from(10u128),
    };
    // Owner set as Cw20ReceiveMsg sender to ensure that this message will error even if a malicious user
    // sends the message directly with the owner address provided
    let receive_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: Uint128::from(100u128),
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Incorrect CW20 provided for sale".to_string()
        }
    )
}

#[test]
pub fn test_start_sale_unauthorised() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate: Uint128::from(10u128),
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: "not_owner".to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: Uint128::from(100u128),
    };
    let msg = ExecuteMsg::Receive(receive_msg);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(err, ContractError::Unauthorized {})
}

#[test]
pub fn test_start_sale_zero_amount() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate: Uint128::from(10u128),
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: "not_owner".to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: Uint128::zero(),
    };
    let msg = ExecuteMsg::Receive(receive_msg);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Cannot send a 0 amount".to_string()
        }
    )
}

#[test]
pub fn test_start_sale() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let token_info = mock_info(token_address.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset.clone(),
        exchange_rate,
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: sale_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    execute(deps.as_mut(), env, token_info, msg).unwrap();

    let sale = SALE
        .load(deps.as_ref().storage, &exchange_asset.to_string())
        .unwrap();

    assert_eq!(sale.exchange_rate, exchange_rate);
    assert_eq!(sale.amount, sale_amount)
}

#[test]
pub fn test_start_sale_ongoing() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let token_info = mock_info(token_address.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate,
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: sale_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    execute(deps.as_mut(), env.clone(), token_info.clone(), msg.clone()).unwrap();

    let err = execute(deps.as_mut(), env, token_info, msg).unwrap_err();

    assert_eq!(err, ContractError::SaleNotEnded {})
}

#[test]
pub fn test_start_sale_zero_exchange_rate() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let token_info = mock_info(token_address.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::zero();
    let sale_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate,
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: sale_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, token_info, msg).unwrap_err();

    assert_eq!(err, ContractError::InvalidZeroAmount {})
}

#[test]
pub fn test_purchase_no_sale() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let token_info = mock_info("invalid_token", &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    // Purchase Tokens
    let purchase_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, token_info, msg).unwrap_err();

    assert_eq!(err, ContractError::NoOngoingSale {});
}

#[test]
pub fn test_purchase_not_enough_sent() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: Uint128::from(100u128),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let exchange_info = mock_info("exchanged_asset", &[]);
    let purchase_amount = Uint128::from(1u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, exchange_info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Not enough funds sent to purchase a token".to_string()
        }
    );
}

#[test]
pub fn test_purchase_no_tokens_left() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: Uint128::zero(),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let exchange_info = mock_info("exchanged_asset", &[]);
    let purchase_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, exchange_info, msg).unwrap_err();

    assert_eq!(err, ContractError::NotEnoughTokens {});
}

#[test]
pub fn test_purchase_not_enough_tokens() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: Uint128::one(),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let exchange_info = mock_info("exchanged_asset", &[]);
    let purchase_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, exchange_info, msg).unwrap_err();

    assert_eq!(err, ContractError::NotEnoughTokens {});
}
