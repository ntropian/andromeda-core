// The MARS auction contract was used as a base for this:
// https://github.com/mars-protocol/mars-periphery/blob/main/contracts/auction/src/contract.rs
use std::ops::Div;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, QueryRequest, Response, StdResult, Storage, Uint128, WasmMsg,
    WasmQuery,
};

use andromeda_protocol::liquidity_bootstrap::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
    StateResponse, UpdateConfigMsg, UserInfoResponse,
};

use andromeda_protocol::lockdrop::ExecuteMsg::EnableClaims as LockdropEnableClaims;
use cw2::set_contract_version;
use cw_asset::AssetInfo as CwAssetInfo;

use astroport::asset::{Asset, AssetInfo};
use astroport::generator::{PendingTokenResponse, QueryMsg as GenQueryMsg};

use crate::{
    primitive_keys::{ADDRESSES_TO_CACHE, ASTROPORT_ASTRO, ASTROPORT_GENERATOR},
    state::{Config, State, UserInfo, CONFIG, STATE, USERS},
};
use ado_base::ADOContract;
use common::{
    ado_base::InstantiateMsg as BaseInstantiateMsg, encode_binary, error::ContractError, require,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

const UUSD_DENOM: &str = "uusd";

// version info for migration info
const CONTRACT_NAME: &str = "andromeda-liquidity-bootstrap";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

//----------------------------------------------------------------------------------------
// Entry points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    require(
        msg.init_timestamp >= env.block.time.seconds(),
        ContractError::StartTimeInThePast {
            current_seconds: env.block.time.seconds(),
            current_block: env.block.height,
        },
    )?;

    require(
        msg.token_deposit_window <= msg.ust_deposit_window,
        ContractError::InvalidWindow {},
    )?;

    require(
        msg.token_vesting_duration > 0,
        ContractError::InvalidVestingDuration {},
    )?;

    let lockdrop_contract_address = msg
        .lockdrop_contract_address
        .map(|v| deps.api.addr_validate(&v))
        // Option<Result> to Result<Option>
        .map_or(Ok(None), |v| v.map(Some));

    let config = Config {
        token_address: deps.api.addr_validate(&msg.token_address)?,
        lockdrop_contract_address: lockdrop_contract_address?,
        lp_token_address: None,
        astroport_lp_pool: None,
        token_lp_staking_contract: None,
        token_rewards: Uint128::zero(),
        token_vesting_duration: msg.token_vesting_duration,
        lp_tokens_vesting_duration: msg.lp_tokens_vesting_duration,
        init_timestamp: msg.init_timestamp,
        token_deposit_window: msg.token_deposit_window,
        ust_deposit_window: msg.ust_deposit_window,
        withdrawal_window: msg.withdrawal_window,
    };

    let state = STATE.load(deps.storage).unwrap_or_default();

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;

    let contract = ADOContract::default();

    let resp = contract.instantiate(
        deps.storage,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "liquidity_bootstrap".to_string(),
            primitive_contract: Some(msg.primitive_contract),
            operators: None,
            modules: None,
        },
    )?;

    for address in ADDRESSES_TO_CACHE {
        contract.cache_address(deps.storage, &deps.querier, address)?;
    }

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig { new_config } => handle_update_config(deps, info, new_config),

        ExecuteMsg::DepositUst {} => handle_deposit_ust(deps, env, info),
        ExecuteMsg::WithdrawUst { amount } => handle_withdraw_ust(deps, env, info, amount),

        ExecuteMsg::AddLiquidityToAstroportPool { slippage } => {
            handle_init_pool(deps, env, info, slippage)
        }
        ExecuteMsg::StakeLpTokens {
            single_incentive_staking,
            dual_incentives_staking,
        } => handle_stake_lp_tokens(
            deps,
            env,
            info,
            single_incentive_staking,
            dual_incentives_staking,
        ),

        ExecuteMsg::ClaimRewards {
            withdraw_unlocked_shares,
        } => handle_claim_rewards_and_unlock(deps, env, info, withdraw_unlocked_shares),

        ExecuteMsg::Callback(msg) => _handle_callback(deps, env, info, msg),
    }
}

/// @dev Receive CW20 hook to accept cw20 token deposits via `Send`. Used to accept MARS  deposits via Airdrop / Lockdrop contracts
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    require(
        info.sender == config.token_address,
        ContractError::InvalidFunds {
            msg: "Invalid cw20 deposited".to_string(),
        },
    )?;

    require(
        !cw20_msg.amount.is_zero(),
        ContractError::InvalidFunds {
            msg: "Amount must be non-zero".to_string(),
        },
    )?;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::DepositTokens { user_address } => {
            // CHECK :: MARS deposits can happen only via lockdrop contract if it is specified
            match config.lockdrop_contract_address {
                None => {}
                Some(address) => {
                    require(address == cw20_msg.sender, ContractError::Unauthorized {})?;
                }
            }

            handle_deposit_tokens(deps, env, info, user_address, cw20_msg.amount)
        }
        Cw20HookMsg::IncreaseIncentives {} => execute_increase_incentives(deps, cw20_msg.amount),
    }
}

fn _handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> Result<Response, ContractError> {
    // Callback functions can only be called this contract itself
    require(
        info.sender == env.contract.address,
        ContractError::Unauthorized {},
    )?;
    match msg {
        CallbackMsg::UpdateStateOnLiquidityAdditionToPool { prev_lp_balance } => {
            update_state_on_liquidity_addition_to_pool(deps, env, prev_lp_balance)
        }
        CallbackMsg::UpdateStateOnRewardClaim {
            user_address,
            prev_mars_balance,
            prev_astro_balance,
            withdraw_lp_shares,
        } => update_state_on_reward_claim(
            deps,
            env,
            user_address,
            prev_mars_balance,
            prev_astro_balance,
            withdraw_lp_shares,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => encode_binary(&query_config(deps)?),
        QueryMsg::State {} => encode_binary(&query_state(deps)?),
        QueryMsg::UserInfo { address } => encode_binary(&query_user_info(deps, env, address)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

//----------------------------------------------------------------------------------------
// Handle functions
//----------------------------------------------------------------------------------------

/// @dev Facilitates increasing MARS incentives which are to be distributed for partcipating in the auction
pub fn execute_increase_incentives(
    deps: DepsMut,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    require(
        state.lp_shares_minted.is_zero(),
        ContractError::TokenAlreadyBeingDistributed {},
    )?;

    config.token_rewards += amount;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "incentives_increased")
        .add_attribute("amount", amount))
}

/// @dev Admin function to update Configuration parameters
/// @param new_config : Same as UpdateConfigMsg struct
pub fn handle_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let contract = ADOContract::default();

    require(
        contract.is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {},
    )?;

    // IF POOL ADDRESS PROVIDED :: Update and query LP token address from the pool
    if let Some(astroport_lp_pool) = new_config.astroport_lp_pool {
        config.astroport_lp_pool = Some(deps.api.addr_validate(&astroport_lp_pool)?);

        let pair_info: astroport::asset::PairInfo = deps
            .querier
            .query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: config.clone().astroport_lp_pool.unwrap().to_string(),
                msg: to_binary(&astroport::pair::QueryMsg::Pair {}).unwrap(),
            }))
            .unwrap();

        config.lp_token_address = Some(pair_info.liquidity_token);
    }

    if let Some(lp_staking_contract) = new_config.lp_staking_contract {
        config.token_lp_staking_contract = Some(deps.api.addr_validate(&lp_staking_contract)?);
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "Auction::ExecuteMsg::UpdateConfig"))
}

/// @dev Accepts MARS tokens to be used for the LP Bootstrapping via auction. Callable only by Airdrop / Lockdrop contracts
/// @param user_address : User address who is delegating the MARS tokens for LP Pool bootstrap via auction
/// @param amount : Number of MARS Tokens being deposited
pub fn handle_deposit_tokens(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    user_address: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: TOKEN delegations window open
    require(
        is_token_deposit_open(env.block.time.seconds(), &config),
        ContractError::DepositWindowClosed {},
    )?;

    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // UPDATE STATE
    state.total_token_deposited += amount;
    user_info.token_deposited += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "deposit_tokens"),
        attr("user", user_address.to_string()),
        attr("tokens_deposited", amount),
    ]))
}

/// @dev Facilitates UST deposits by users to be used for LP Bootstrapping via auction
pub fn handle_deposit_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    require(
        is_ust_deposit_open(env.block.time.seconds(), &config),
        ContractError::DepositWindowClosed {},
    )?;

    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    require(
        info.funds.len() == 1,
        ContractError::InvalidFunds {
            msg: "Can only deposit a single coin".to_string(),
        },
    )?;

    // Only UST accepted and amount > 0
    let native_token = info.funds.first().unwrap();
    require(
        native_token.denom == UUSD_DENOM,
        ContractError::InvalidFunds {
            msg: "Only UST among native tokens accepted".to_string(),
        },
    )?;

    require(
        !native_token.amount.is_zero(),
        ContractError::InvalidFunds {
            msg: "Deposit amount must be greater than 0".to_string(),
        },
    )?;

    // UPDATE STATE
    state.total_ust_deposited += native_token.amount;
    user_info.ust_deposited += native_token.amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &info.sender, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::deposit_ust"),
        attr("user_address", info.sender),
        attr("ust_deposited", native_token.amount),
    ]))
}

/// @dev Facilitates UST withdrawals by users from their deposit positions
/// @param amount : UST amount being withdrawn
pub fn handle_withdraw_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let user_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: Has the user already withdrawn during the current window
    require(
        !user_info.ust_withdrawn_flag,
        ContractError::InvalidWithdrawal {
            msg: Some("Max 1 withdrawal allowed during current window".to_string()),
        },
    )?;

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent = allowed_withdrawal_percent(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = user_info.ust_deposited * max_withdrawal_percent;

    require(
        amount <= max_withdrawal_allowed,
        ContractError::InvalidWithdrawal {
            msg: Some("Amount exceeds maximum allowed withdrawal limit of {} uusd".to_string()),
        },
    )?;

    // After UST deposit window is closed, we allow to withdraw only once
    if env.block.time.seconds() > config.init_timestamp + config.ust_deposit_window {
        user_info.ust_withdrawn_flag = true;
    }

    // UPDATE STATE
    state.total_ust_deposited = state.total_ust_deposited.checked_sub(amount)?;
    user_info.ust_deposited = user_info.ust_deposited.checked_sub(amount)?;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    // COSMOSMSG :: Transfer UST to the user
    let transfer_ust = Asset {
        amount,
        info: AssetInfo::NativeToken {
            denom: String::from(UUSD_DENOM),
        },
    }
    .into_msg(&deps.querier, user_address.clone())?;

    Ok(Response::new()
        .add_message(transfer_ust)
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::withdraw_ust"),
            attr("user", user_address.to_string()),
            attr("ust_withdrawn_flag", amount),
        ]))
}

/// @dev Admin function to bootstrap the MARS-UST Liquidity pool by depositing all MARS, UST tokens deposited to the Astroport pool
/// @param slippage Optional, to handle slippage that may be there when adding liquidity to the pool
pub fn handle_init_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    slippage: Option<Decimal>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Only admin can call this function
    require(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {},
    )?;

    require(
        state.lp_shares_minted.is_zero(),
        ContractError::LiquidityAlreadyProvided {},
    )?;

    require(
        are_windows_closed(env.block.time.seconds(), &config),
        ContractError::WindowsStillOpen {},
    )?;

    require(
        config.astroport_lp_pool.is_some(),
        ContractError::LpAddressNotSet {},
    )?;

    // Init response
    let mut response =
        Response::new().add_attribute("action", "Auction::ExecuteMsg::AddLiquidityToAstroportPool");

    let lp_token = CwAssetInfo::cw20(config.lp_token_address.clone().unwrap());
    // QUERY CURRENT LP TOKEN BALANCE (FOR SAFETY - IN ANY CASE)
    let cur_lp_balance = lp_token.query_balance(&deps.querier, env.contract.address.clone())?;

    // COSMOS MSGS
    // :: 1. APPROVE TOKEN WITH LP POOL ADDRESS AS BENEFICIARY
    // :: 2. ADD LIQUIDITY
    // :: 3. CallbackMsg :: Update state on liquidity addition to LP Pool
    // :: 4. Activate Claims on Lockdrop Contract (In Callback)
    // :: 5. Update Claims on Airdrop Contract (In Callback)
    let approve_token_msg = build_approve_cw20_msg(
        config.token_address.to_string(),
        config.astroport_lp_pool.clone().unwrap().to_string(),
        state.total_token_deposited,
    )?;
    let add_liquidity_msg =
        build_provide_liquidity_to_lp_pool_msg(deps.as_ref(), config, &state, slippage)?;

    let update_state_msg = CallbackMsg::UpdateStateOnLiquidityAdditionToPool {
        prev_lp_balance: cur_lp_balance,
    }
    .to_cosmos_msg(&env.contract.address)?;

    response = response
        .add_messages(vec![approve_token_msg, add_liquidity_msg, update_state_msg])
        .add_attribute("token_deposited", state.total_token_deposited)
        .add_attribute("ust_deposited", state.total_ust_deposited);

    Ok(response)
}

/// @dev Admin function to stake Astroport LP tokens with the generator contract
/// @params single_incentive_staking : Boolean value indicating if LP Tokens are to be staked with MARS LP Contract or not
pub fn handle_stake_lp_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    single_incentive_staking: bool,
    dual_incentives_staking: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let contract = ADOContract::default();
    let mut state = STATE.load(deps.storage)?;
    let mut are_being_unstaked = false;

    require(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {},
    )?;

    // CHECK :: Check if valid boolean values are provided or not
    require(
        single_incentive_staking != dual_incentives_staking,
        ContractError::InvalidValues {},
    )?;

    // Init response
    let mut response =
        Response::new().add_attribute("action", "Auction::ExecuteMsg::StakeLPTokens");

    // CHECK :: Check if already staked with MARS LP Staking contracts
    require(
        !(single_incentive_staking && state.are_staked_for_single_incentives),
        ContractError::StakingError {
            msg: "LP Tokens already staked with MARS LP Staking contract".to_string(),
        },
    )?;

    // CHECK :: Check if already staked with MARS Generator
    require(
        !(dual_incentives_staking && state.are_staked_for_dual_incentives),
        ContractError::StakingError {
            msg: "LP Tokens already staked with Astroport Generator".to_string(),
        },
    )?;

    // IF TO BE STAKED WITH MARS LP STAKING CONTRACT
    if single_incentive_staking {
        let lp_shares_balance = state.lp_shares_minted - state.lp_shares_withdrawn;

        // Unstake from Generator contract (if staked)
        if state.are_staked_for_dual_incentives {
            response = response
                .add_message(build_unstake_from_generator_msg(
                    deps.storage,
                    &config,
                    lp_shares_balance,
                )?)
                .add_attribute(
                    "shares_withdrawn_from_generator",
                    lp_shares_balance.to_string(),
                );
            are_being_unstaked = true;
        }

        // Check if LP Staking contract is set
        require(
            config.token_lp_staking_contract.is_some(),
            ContractError::StakingError {
                msg: "LP Staking not set".to_string(),
            },
        )?;

        // :: Add stake LP Tokens to the MARS LP Staking contract msg
        let stake_msg =
            build_stake_with_mars_staking_contract_msg(config.clone(), lp_shares_balance)?;

        response = response
            .add_message(stake_msg)
            .add_attribute("shares_staked_with_lp_contract", "true")
            .add_attribute("shares_staked_amount", lp_shares_balance.to_string());

        // Update boolean values which indicate where the LP tokens are staked
        state.are_staked_for_single_incentives = true;
        state.are_staked_for_dual_incentives = false;
    }

    // IF TO BE STAKED WITH GENERATOR
    if dual_incentives_staking {
        let lp_shares_balance = state.lp_shares_minted - state.lp_shares_withdrawn;

        // Unstake from LP Staking contract (if staked)
        if state.are_staked_for_single_incentives {
            response = response
                .add_message(build_unstake_from_staking_contract_msg(
                    config
                        .token_lp_staking_contract
                        .clone()
                        .expect("LP Staking contract not set")
                        .to_string(),
                    lp_shares_balance,
                )?)
                .add_attribute(
                    "shares_unstaked_from_lp_staking",
                    lp_shares_balance.to_string(),
                );
            are_being_unstaked = true;
        }

        // COSMOS MSGs
        // :: Add increase allowance msg so generator contract can transfer tokens to itself
        // :: Add stake LP Tokens to the Astroport generator contract msg
        let generator_address = contract.get_cached_address(deps.storage, ASTROPORT_GENERATOR)?;
        let approve_msg = build_approve_cw20_msg(
            config.lp_token_address.clone().unwrap().to_string(),
            generator_address,
            lp_shares_balance,
        )?;
        let stake_msg =
            build_stake_with_generator_msg(deps.storage, config.clone(), lp_shares_balance)?;
        response = response
            .add_messages(vec![approve_msg, stake_msg])
            .add_attribute("shares_staked_with_generator", "true")
            .add_attribute("shares_staked_amount", lp_shares_balance.to_string());

        // Update boolean values which indicate where the LP tokens are staked
        state.are_staked_for_dual_incentives = true;
        state.are_staked_for_single_incentives = false;
    }

    if are_being_unstaked {
        // --> Add CallbackMsg::UpdateStateOnRewardClaim msg to the cosmos msg array
        let token = CwAssetInfo::cw20(config.token_address);
        let token_balance = token.query_balance(&deps.querier, env.contract.address.clone())?;

        let astro_token_address = contract.get_cached_address(deps.storage, ASTROPORT_ASTRO)?;
        let astro_token = CwAssetInfo::cw20(deps.api.addr_validate(&astro_token_address)?);
        let astro_balance =
            astro_token.query_balance(&deps.querier, env.contract.address.clone())?;

        let update_state_msg = CallbackMsg::UpdateStateOnRewardClaim {
            user_address: None,
            prev_mars_balance: token_balance,
            prev_astro_balance: astro_balance,
            withdraw_lp_shares: Uint128::zero(),
        }
        .to_cosmos_msg(&env.contract.address)?;
        response = response.add_message(update_state_msg);
    }

    STATE.save(deps.storage, &state)?;

    Ok(response)
}

/// @dev Facilitates MARS/ASTRO Reward claim for users
/// @params withdraw_unlocked_shares : Boolean value indicating if the vested Shares are to be withdrawn or not
pub fn handle_claim_rewards_and_unlock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_unlocked_shares: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    let user_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: Deposit / withdrawal windows need to be over
    require(
        are_windows_closed(env.block.time.seconds(), &config),
        ContractError::WindowsStillOpen {},
    )?;

    // CHECK :: Does user have valid MARS / UST deposit balances
    require(
        !user_info.token_deposited.is_zero() || !user_info.ust_deposited.is_zero(),
        ContractError::InvalidValues {},
    )?;

    // Init response
    let mut response = Response::new()
        .add_attribute("action", "Auction::ExecuteMsg::ClaimRewards")
        .add_attribute("user_address", user_address.to_string())
        .add_attribute("withdraw_lp_shares", withdraw_unlocked_shares.to_string());

    // LP SHARES :: Calculate if not already calculated
    if user_info.lp_shares == Uint128::zero() {
        user_info.lp_shares = calculate_user_lp_share(&state, &user_info);
        response = response.add_attribute("user_lp_share", user_info.lp_shares.to_string());
    }

    // TOKEN INCENTIVES :: Calculates TOKEN rewards for auction participation for a user if not already done
    if user_info.total_auction_incentives == Uint128::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.token_rewards);
        response = response.add_attribute(
            "user_total_auction_mars_incentive",
            user_info.total_auction_incentives.to_string(),
        );
    }

    let mut lp_shares_to_withdraw = Uint128::zero();
    if withdraw_unlocked_shares {
        lp_shares_to_withdraw =
            calculate_withdrawable_lp_shares(env.block.time.seconds(), &config, &state, &user_info);
    }

    // --> IF LP TOKENS are staked with MARS LP Staking contract
    if state.are_staked_for_single_incentives {
        let pending_token_rewards = query_unclaimed_staking_rewards_at_lp_staking(
            &deps.querier,
            config
                .token_lp_staking_contract
                .clone()
                .expect("LP Staking contract not set")
                .to_string(),
            config.token_address.as_str(),
            env.contract.address.clone(),
        )?;

        if pending_token_rewards > Uint128::zero() || withdraw_unlocked_shares {
            let claim_reward_msg: CosmosMsg;
            // If LP tokens are to be withdrawn. We unstake the equivalent amount. Rewards are automatically claimed with the call
            if withdraw_unlocked_shares {
                claim_reward_msg = build_unstake_from_staking_contract_msg(
                    config
                        .token_lp_staking_contract
                        .clone()
                        .expect("LP Staking contract not set")
                        .to_string(),
                    lp_shares_to_withdraw,
                )?;
            }
            // If only rewards are to be claimed
            else {
                claim_reward_msg = build_claim_rewards_from_mars_staking_contract_msg(
                    config
                        .token_lp_staking_contract
                        .clone()
                        .expect("LP Staking contract not set")
                        .to_string(),
                )?;
            }
            response = response
                .add_message(claim_reward_msg)
                .add_attribute("claim_rewards", "mars_staking_contract");
        }
    }

    if state.are_staked_for_dual_incentives {
        let unclaimed_rewards_response: astroport::generator::PendingTokenResponse =
            query_unclaimed_staking_rewards_at_generator(
                deps.storage,
                &deps.querier,
                &config,
                env.contract.address.clone(),
            )?;

        if unclaimed_rewards_response.pending > Uint128::zero()
            || (unclaimed_rewards_response.pending_on_proxy.is_some()
                && unclaimed_rewards_response.pending_on_proxy.unwrap() > Uint128::zero())
            || withdraw_unlocked_shares
        {
            let claim_reward_msg =
                build_unstake_from_generator_msg(deps.storage, &config, lp_shares_to_withdraw)?;
            response = response
                .add_message(claim_reward_msg)
                .add_attribute("claim_rewards", "generator");
        }
    }

    let contract = ADOContract::default();
    // --> Add CallbackMsg::UpdateStateOnRewardClaim msg to the cosmos msg array
    let token = CwAssetInfo::cw20(config.token_address);
    let token_balance = token.query_balance(&deps.querier, env.contract.address.clone())?;

    let astro_token_address = contract.get_cached_address(deps.storage, ASTROPORT_ASTRO)?;
    let astro_token = CwAssetInfo::cw20(deps.api.addr_validate(&astro_token_address)?);
    let astro_balance = astro_token.query_balance(&deps.querier, env.contract.address.clone())?;

    USERS.save(deps.storage, &user_address, &user_info)?;

    let update_state_msg = CallbackMsg::UpdateStateOnRewardClaim {
        user_address: Some(user_address),
        prev_mars_balance: token_balance,
        prev_astro_balance: astro_balance,
        withdraw_lp_shares: lp_shares_to_withdraw,
    }
    .to_cosmos_msg(&env.contract.address)?;
    response = response.add_message(update_state_msg);

    Ok(response)
}

//----------------------------------------------------------------------------------------
// Handle::Callback functions
//----------------------------------------------------------------------------------------

/// @dev Callback function. Updates state after initialization of MARS-UST Pool
/// @params prev_lp_balance : Astro LP Token balance before pool initialization
pub fn update_state_on_liquidity_addition_to_pool(
    deps: DepsMut,
    env: Env,
    prev_lp_balance: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // QUERY CURRENT LP TOKEN BALANCE :: NEWLY MINTED LP TOKENS
    let lp_token = CwAssetInfo::cw20(config.lp_token_address.expect("LP Token not set"));
    let cur_lp_balance = lp_token.query_balance(&deps.querier, env.contract.address)?;

    // STATE :: UPDATE --> SAVE
    state.lp_shares_minted = cur_lp_balance - prev_lp_balance;
    state.pool_init_timestamp = env.block.time.seconds();
    STATE.save(deps.storage, &state)?;

    let mut cosmos_msgs = vec![];
    if let Some(lockdrop_contract_address) = config.lockdrop_contract_address {
        let activate_claims_lockdrop =
            build_activate_claims_lockdrop_msg(lockdrop_contract_address)?;
        cosmos_msgs.push(activate_claims_lockdrop);
    }

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            (
                "action",
                "Auction::CallbackMsg::UpdateStateOnLiquidityAddition",
            ),
            (
                "lp_shares_minted",
                state.lp_shares_minted.to_string().as_str(),
            ),
        ]))
}

// @dev CallbackMsg :: Facilitates state update and MARS / ASTRO rewards transfer to users post MARS incentives claim from the generator contract
pub fn update_state_on_reward_claim(
    deps: DepsMut,
    env: Env,
    user_address: Option<Addr>,
    prev_mars_balance: Uint128,
    prev_astro_balance: Uint128,
    withdraw_lp_shares: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let contract = ADOContract::default();

    // Claimed Rewards :: QUERY TOKEN & ASTRO TOKEN BALANCE
    let token = CwAssetInfo::cw20(config.token_address.clone());
    let cur_token_balance = token.query_balance(&deps.querier, env.contract.address.clone())?;

    let astro_token_address = contract.get_cached_address(deps.storage, ASTROPORT_ASTRO)?;
    let astro_token = CwAssetInfo::cw20(deps.api.addr_validate(&astro_token_address)?);
    let cur_astro_balance =
        astro_token.query_balance(&deps.querier, env.contract.address.clone())?;

    let mars_claimed = cur_token_balance.checked_sub(prev_mars_balance)?;
    let astro_claimed = cur_astro_balance.checked_sub(prev_astro_balance)?;

    // Update Global Reward Indexes
    update_mars_rewards_index(&mut state, mars_claimed);
    update_astro_rewards_index(&mut state, astro_claimed);

    // Init response
    let mut response = Response::new()
        .add_attribute("total_claimed_mars", mars_claimed.to_string())
        .add_attribute("total_claimed_astro", astro_claimed.to_string());

    // IF VALID USER ADDRESSES (All cases except staking() function call)
    if let Some(user_address) = user_address {
        let mut user_info = USERS
            .may_load(deps.storage, &user_address)?
            .unwrap_or_default();

        // MARS Incentives :: Calculate the unvested amount which can be claimed by the user
        let mut user_mars_rewards = calculate_withdrawable_auction_reward_for_user(
            env.block.time.seconds(),
            &config,
            &state,
            &user_info,
        );
        user_info.withdrawn_auction_incentives += user_mars_rewards;
        response = response.add_attribute(
            "withdrawn_auction_incentives",
            user_mars_rewards.to_string(),
        );

        // MARS (Staking) rewards :: Calculate the amount (from LP staking incentives) which can be claimed by the user
        let staking_reward_token = compute_user_accrued_mars_reward(&state, &mut user_info);
        user_info.withdrawn_token_incentives += staking_reward_token;
        user_mars_rewards += staking_reward_token;
        response = response.add_attribute("user_mars_incentives", staking_reward_token.to_string());

        // ASTRO (Staking) rewards :: Calculate the amount (from LP staking incentives) which can be claimed by the user
        let staking_reward_astro = compute_user_accrued_astro_reward(&state, &mut user_info);
        user_info.withdrawn_astro_incentives += staking_reward_astro;
        response =
            response.add_attribute("user_astro_incentives", staking_reward_astro.to_string());

        // COSMOS MSG :: Transfer $MARS to the user
        if user_mars_rewards > Uint128::zero() {
            let transfer_mars_rewards = build_transfer_cw20_token_msg(
                user_address.clone(),
                config.token_address.to_string(),
                user_mars_rewards,
            )?;
            response = response.add_message(transfer_mars_rewards);
        }

        // COSMOS MSG :: Transfer $ASTRO to the user
        let astro_token_address = contract.get_cached_address(deps.storage, ASTROPORT_ASTRO)?;
        if staking_reward_astro > Uint128::zero() {
            let transfer_astro_rewards = build_transfer_cw20_token_msg(
                user_address.clone(),
                astro_token_address,
                staking_reward_astro,
            )?;
            response = response.add_message(transfer_astro_rewards);
        }

        // COSMOS MSG :: WITHDRAW LP Shares
        if withdraw_lp_shares > Uint128::zero() {
            let transfer_lp_shares = build_transfer_cw20_token_msg(
                user_address.clone(),
                config
                    .lp_token_address
                    .expect("LP Token not set")
                    .to_string(),
                withdraw_lp_shares,
            )?;
            response = response.add_message(transfer_lp_shares);

            user_info.withdrawn_lp_shares += withdraw_lp_shares;
            state.lp_shares_withdrawn += withdraw_lp_shares;
        }

        USERS.save(deps.storage, &user_address, &user_info)?;
    }

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;

    Ok(response)
}

//----------------------------------------------------------------------------------------
// Query functions
//----------------------------------------------------------------------------------------

/// @dev Returns the airdrop configuration
fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        token_address: config.token_address.to_string(),
        lockdrop_contract_address: config.lockdrop_contract_address,
        astroport_lp_pool: config.astroport_lp_pool,
        lp_token_address: config.lp_token_address,
        token_lp_staking_contract: config.token_lp_staking_contract,
        token_rewards: config.token_rewards,
        token_vesting_duration: config.token_vesting_duration,
        lp_tokens_vesting_duration: config.lp_tokens_vesting_duration,
        init_timestamp: config.init_timestamp,
        token_deposit_window: config.token_deposit_window,
        ust_deposit_window: config.ust_deposit_window,
        withdrawal_window: config.withdrawal_window,
    })
}

/// @dev Returns the airdrop contract state
fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_token_deposited: state.total_token_deposited,
        total_ust_deposited: state.total_ust_deposited,
        lp_shares_minted: state.lp_shares_minted,
        lp_shares_withdrawn: state.lp_shares_withdrawn,
        are_staked_for_single_incentives: state.are_staked_for_single_incentives,
        are_staked_for_dual_incentives: state.are_staked_for_dual_incentives,
        pool_init_timestamp: state.pool_init_timestamp,
        global_token_reward_index: state.global_token_reward_index,
        global_astro_reward_index: state.global_astro_reward_index,
    })
}

/// @dev Returns details around user's MARS Airdrop claim
fn query_user_info(
    deps: Deps,
    env: Env,
    user_address: String,
) -> Result<UserInfoResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = deps.api.addr_validate(&user_address)?;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    if user_info.lp_shares == Uint128::zero() {
        user_info.lp_shares = calculate_user_lp_share(&state, &user_info);
    }

    if user_info.total_auction_incentives == Uint128::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.token_rewards);
    }
    let withdrawable_lp_shares =
        calculate_withdrawable_lp_shares(env.block.time.seconds(), &config, &state, &user_info);
    let claimable_auction_reward = calculate_withdrawable_auction_reward_for_user(
        env.block.time.seconds(),
        &config,
        &state,
        &user_info,
    );

    let mut withdrawable_token_incentives = Uint128::zero();
    let mut withdrawable_astro_incentives = Uint128::zero();

    // --> IF LP TOKENS are staked with MARS LP STaking contract
    if state.are_staked_for_single_incentives {
        let pending_token_rewards = query_unclaimed_staking_rewards_at_lp_staking(
            &deps.querier,
            config
                .token_lp_staking_contract
                .clone()
                .expect("LP Staking contract not set")
                .to_string(),
            config.token_address.as_str(),
            env.contract.address.clone(),
        )?;
        update_mars_rewards_index(&mut state, pending_token_rewards);
        withdrawable_token_incentives = compute_user_accrued_mars_reward(&state, &mut user_info);
    }

    // --> IF LP TOKENS are staked with Generator contract
    if state.are_staked_for_dual_incentives {
        let unclaimed_rewards_response = query_unclaimed_staking_rewards_at_generator(
            deps.storage,
            &deps.querier,
            &config,
            env.contract.address,
        )?;
        update_mars_rewards_index(
            &mut state,
            unclaimed_rewards_response.pending_on_proxy.unwrap(),
        );
        withdrawable_token_incentives = compute_user_accrued_mars_reward(&state, &mut user_info);

        update_astro_rewards_index(&mut state, unclaimed_rewards_response.pending);
        withdrawable_astro_incentives = compute_user_accrued_astro_reward(&state, &mut user_info);
    }

    Ok(UserInfoResponse {
        token_deposited: user_info.token_deposited,
        ust_deposited: user_info.ust_deposited,
        ust_withdrawn_flag: user_info.ust_withdrawn_flag,
        lp_shares: user_info.lp_shares,
        withdrawn_lp_shares: user_info.withdrawn_lp_shares,
        withdrawable_lp_shares,
        total_auction_incentives: user_info.total_auction_incentives,
        withdrawn_auction_incentives: user_info.withdrawn_auction_incentives,
        withdrawable_auction_incentives: claimable_auction_reward,
        token_reward_index: user_info.token_reward_index,
        withdrawable_token_incentives,
        withdrawn_token_incentives: user_info.withdrawn_token_incentives,
        astro_reward_index: user_info.astro_reward_index,
        withdrawable_astro_incentives,
        withdrawn_astro_incentives: user_info.withdrawn_astro_incentives,
    })
}

//----------------------------------------------------------------------------------------
// HELPERS :: LP & REWARD CALCULATIONS
//----------------------------------------------------------------------------------------

/// @dev Calculates user's MARS-UST LP Shares
/// Formula -
/// user's MARS share %  = user's MARS deposits / Total MARS deposited
/// user's UST share %  = user's UST deposits / Total UST deposited
/// user's LP balance  = ( user's MARS share % + user's UST share % ) / 2 * Total LPs Minted
/// @param state : Contract State
/// @param user_info : User Info State
fn calculate_user_lp_share(state: &State, user_info: &UserInfo) -> Uint128 {
    if state.total_token_deposited == Uint128::zero()
        || state.total_ust_deposited == Uint128::zero()
    {
        return user_info.lp_shares;
    }
    let user_token_shares_percent =
        Decimal::from_ratio(user_info.token_deposited, state.total_token_deposited);
    let user_ust_shares_percent =
        Decimal::from_ratio(user_info.ust_deposited, state.total_ust_deposited);
    let user_total_share_percent = user_token_shares_percent + user_ust_shares_percent;

    user_total_share_percent.div(Uint128::from(2u64)) * state.lp_shares_minted
}

/// @dev Calculates MARS tokens receivable by a user for delegating MARS & depositing UST in the bootstraping phase of the MARS-UST Pool
/// Formula -
/// user's MARS share %  = user's MARS deposits / Total MARS deposited
/// user's UST share %  = user's UST deposits / Total UST deposited
/// user's Auction Reward  = ( user's MARS share % + user's UST share % ) / 2 * Total Auction Incentives
/// @param total_mars_rewards : Total MARS tokens to be distributed as auction participation reward
fn calculate_auction_reward_for_user(
    state: &State,
    user_info: &UserInfo,
    total_token_rewards: Uint128,
) -> Uint128 {
    let mut user_token_shares_percent = Decimal::zero();
    let mut user_ust_shares_percent = Decimal::zero();

    if user_info.token_deposited > Uint128::zero() {
        user_token_shares_percent =
            Decimal::from_ratio(user_info.token_deposited, state.total_token_deposited);
    }
    if user_info.ust_deposited > Uint128::zero() {
        user_ust_shares_percent =
            Decimal::from_ratio(user_info.ust_deposited, state.total_ust_deposited);
    }
    let user_total_share_percent = user_token_shares_percent + user_ust_shares_percent;
    user_total_share_percent.div(Uint128::from(2u64)) * total_token_rewards
}

/// @dev Returns LP Balance that a user can withdraw based on the vesting schedule
/// Formula -
/// time elapsed = current timestamp - timestamp when liquidity was added to the MARS-UST LP Pool
/// Total LP shares that a user can withdraw =  User's LP shares *  time elapsed / vesting duration
/// LP shares that a user can currently withdraw =  Total LP shares that a user can withdraw  - LP shares withdrawn
/// @param current_timestamp : Current timestamp
/// @param user_info : User Info State
pub fn calculate_withdrawable_lp_shares(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint128 {
    if state.pool_init_timestamp == 0u64 {
        return Uint128::zero();
    }
    let time_elapsed = cur_timestamp - state.pool_init_timestamp;

    if time_elapsed >= config.lp_tokens_vesting_duration {
        return user_info.lp_shares - user_info.withdrawn_lp_shares;
    }

    let withdrawable_lp_balance =
        user_info.lp_shares * Decimal::from_ratio(time_elapsed, config.lp_tokens_vesting_duration);
    withdrawable_lp_balance - user_info.withdrawn_lp_shares
}

/// @dev Returns MARS auction incentives that a user can withdraw based on the vesting schedule
/// Formula -
/// time elapsed = current timestamp - timestamp when liquidity was added to the MARS-UST LP Pool
/// Total MARS that a user can withdraw =  User's MARS reward *  time elapsed / vesting duration
/// MARS rewards that a user can currently withdraw =  Total MARS rewards that a user can withdraw  - MARS rewards withdrawn
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
/// @param state : Contract State
/// @param user_info : User Info State
pub fn calculate_withdrawable_auction_reward_for_user(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint128 {
    if user_info.withdrawn_auction_incentives == user_info.total_auction_incentives
        || state.pool_init_timestamp == 0u64
    {
        return Uint128::zero();
    }

    let time_elapsed = cur_timestamp - state.pool_init_timestamp;
    if time_elapsed >= config.token_vesting_duration {
        return user_info.total_auction_incentives - user_info.withdrawn_auction_incentives;
    }
    let withdrawable_auction_incentives = user_info.total_auction_incentives
        * Decimal::from_ratio(time_elapsed, config.token_vesting_duration);
    withdrawable_auction_incentives - user_info.withdrawn_auction_incentives
}

/// @dev Accrue MARS rewards by updating the global mars reward index
/// Formula ::: global mars reward index += MARS accrued / (LP shares staked)
fn update_mars_rewards_index(state: &mut State, token_accured: Uint128) {
    let staked_lp_shares = state.lp_shares_minted - state.lp_shares_withdrawn;
    if staked_lp_shares == Uint128::zero() {
        return;
    }
    state.global_token_reward_index =
        state.global_token_reward_index + Decimal::from_ratio(token_accured, staked_lp_shares);
}

/// @dev Accrue ASTRO rewards by updating the global astro reward index
/// Formula ::: global astro reward index += ASTRO accrued / (LP shares staked)
fn update_astro_rewards_index(state: &mut State, astro_accured: Uint128) {
    let staked_lp_shares = state.lp_shares_minted - state.lp_shares_withdrawn;
    if staked_lp_shares == Uint128::zero() {
        return;
    }
    state.global_astro_reward_index =
        state.global_astro_reward_index + Decimal::from_ratio(astro_accured, staked_lp_shares);
}

/// @dev Accrue MARS reward for the user by updating the user reward index and adding rewards to the pending rewards
/// Formula :: Pending user mars rewards = (user's staked LP shares) * ( global mars reward index - user mars reward index )
fn compute_user_accrued_mars_reward(state: &State, user_info: &mut UserInfo) -> Uint128 {
    let staked_lp_shares = user_info.lp_shares - user_info.withdrawn_lp_shares;

    let pending_user_rewards = (staked_lp_shares * state.global_token_reward_index)
        - (staked_lp_shares * user_info.token_reward_index);
    user_info.token_reward_index = state.global_token_reward_index;
    pending_user_rewards
}

/// @dev Accrue ASTRO reward for the user by updating the user reward index
/// Formula :: Pending user astro rewards = (user's staked LP shares) * ( global astro reward index - user astro reward index )
fn compute_user_accrued_astro_reward(state: &State, user_info: &mut UserInfo) -> Uint128 {
    let staked_lp_shares = user_info.lp_shares - user_info.withdrawn_lp_shares;
    let pending_user_rewards = (staked_lp_shares * state.global_astro_reward_index)
        - (staked_lp_shares * user_info.astro_reward_index);
    user_info.astro_reward_index = state.global_astro_reward_index;
    pending_user_rewards
}

//----------------------------------------------------------------------------------------
// HELPERS :: DEPOSIT / WITHDRAW CALCULATIONS
//----------------------------------------------------------------------------------------

/// @dev Helper function. Returns true if the deposit & withdrawal windows are closed, else returns false
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
fn are_windows_closed(current_timestamp: u64, config: &Config) -> bool {
    let opened_till = config.init_timestamp + config.ust_deposit_window + config.withdrawal_window;
    (current_timestamp > opened_till) || (current_timestamp < config.init_timestamp)
}

///  @dev Helper function to calculate maximum % of their total UST deposited that can be withdrawn.  Returns % UST that can be withdrawn
/// @params current_timestamp : Current block timestamp
/// @params config : Contract configuration
fn allowed_withdrawal_percent(current_timestamp: u64, config: &Config) -> Decimal {
    let ust_withdrawal_cutoff_init_point = config.init_timestamp + config.ust_deposit_window;

    // Deposit window :: 100% withdrawals allowed
    if current_timestamp <= ust_withdrawal_cutoff_init_point {
        return Decimal::from_ratio(100u32, 100u32);
    }

    let ust_withdrawal_cutoff_second_point =
        ust_withdrawal_cutoff_init_point + (config.withdrawal_window / 2u64);
    // Deposit window closed, 1st half of withdrawal window :: 50% withdrawals allowed
    if current_timestamp <= ust_withdrawal_cutoff_second_point {
        return Decimal::from_ratio(50u32, 100u32);
    }
    let ust_withdrawal_cutoff_final =
        ust_withdrawal_cutoff_second_point + (config.withdrawal_window / 2u64);
    //  Deposit window closed, 2nd half of withdrawal window :: max withdrawal allowed decreases linearly from 50% to 0% vs time elapsed
    if current_timestamp < ust_withdrawal_cutoff_final {
        let time_left = ust_withdrawal_cutoff_final - current_timestamp;
        Decimal::from_ratio(
            50u64 * time_left,
            100u64 * (ust_withdrawal_cutoff_final - ust_withdrawal_cutoff_second_point),
        )
    }
    // Withdrawals not allowed
    else {
        Decimal::from_ratio(0u32, 100u32)
    }
}

/// @dev Returns true if deposits are allowed
fn is_token_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.token_deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}

/// @dev Returns true if deposits are allowed
fn is_ust_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.ust_deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}

//----------------------------------------------------------------------------------------
// HELPERS :: QUERIES
//----------------------------------------------------------------------------------------

/// @dev Queries pending rewards to be claimed from the generator contract for the 'contract_addr'
/// @param config : Configuration
/// @param contract_addr : Address for which pending rewards are to be queried
fn query_unclaimed_staking_rewards_at_generator(
    storage: &dyn Storage,
    querier: &QuerierWrapper,
    config: &Config,
    contract_addr: Addr,
) -> Result<astroport::generator::PendingTokenResponse, ContractError> {
    let generator_contract =
        ADOContract::default().get_cached_address(storage, ASTROPORT_GENERATOR)?;
    let pending_rewards: PendingTokenResponse =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: generator_contract,
            msg: to_binary(&GenQueryMsg::PendingToken {
                lp_token: config.lp_token_address.clone().expect("LP Token not set"),
                user: contract_addr,
            })?,
        }))?;
    Ok(pending_rewards)
}

/// @dev Queries pending rewards to be claimed from the MARS LP Staking contract
/// @param config : Configuration
/// @param contract_addr : Address for which pending rewards are to be queried
fn query_unclaimed_staking_rewards_at_lp_staking(
    querier: &QuerierWrapper,
    mars_lp_staking_contract: String,
    token_address: &str,
    contract_addr: Addr,
) -> Result<Uint128, ContractError> {
    let unclaimed_rewards_response: andromeda_protocol::cw20_staking::StakerResponse = querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: mars_lp_staking_contract,
            msg: to_binary(&andromeda_protocol::cw20_staking::QueryMsg::Staker {
                address: contract_addr.to_string(),
            })?,
        }))?;
    let pending_token_rewards = unclaimed_rewards_response
        .pending_rewards
        .into_iter()
        .find(|t| t.0 == token_address);

    match pending_token_rewards {
        Some(pending_token_rewards) => Ok(pending_token_rewards.1),
        None => Err(ContractError::StakingError {
            msg: format!("{} is not a valid reward in the vault", token_address),
        }),
    }
}

//----------------------------------------------------------------------------------------
// HELPERS :: BUILD COSMOS MSG
//----------------------------------------------------------------------------------------

/// @dev Returns CosmosMsg struct to stake LP Tokens with the MARS LP Staking contract
/// @param amount : LP tokens to stake
pub fn build_stake_with_mars_staking_contract_msg(
    config: Config,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    let stake_msg = to_binary(&andromeda_protocol::cw20_staking::Cw20HookMsg::StakeTokens {})?;
    build_send_cw20_token_msg(
        config
            .token_lp_staking_contract
            .expect("LP Staking address not set")
            .to_string(),
        config
            .lp_token_address
            .expect("LP Token address not set")
            .to_string(),
        amount,
        stake_msg,
    )
}

/// @dev Returns CosmosMsg struct to unstake LP Tokens from MARS LP Staking contract
/// @param config : Configuration
/// @param amount : LP tokens to unstake
/// @param claim_rewards : Boolean value indicating is Rewards are to be claimed or not
pub fn build_unstake_from_staking_contract_msg(
    mars_lp_staking_contract: String,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: mars_lp_staking_contract,
        msg: to_binary(
            &andromeda_protocol::cw20_staking::ExecuteMsg::UnstakeTokens {
                amount: Some(amount),
            },
        )?,
        funds: vec![],
    }))
}

/// @dev Returns CosmosMsg struct to claim MARS from MARS LP Staking contract
/// @param mars_lp_staking_contract : Mars LP Staking contract
pub fn build_claim_rewards_from_mars_staking_contract_msg(
    mars_lp_staking_contract: String,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: mars_lp_staking_contract,
        msg: to_binary(&andromeda_protocol::cw20_staking::ExecuteMsg::ClaimRewards {})?,
        funds: vec![],
    }))
}

/// @dev Returns CosmosMsg struct to stake LP Tokens with the Generator contract
/// @param config : Configuration
/// @param amount : LP tokens to stake
pub fn build_stake_with_generator_msg(
    storage: &dyn Storage,
    config: Config,
    amount: Uint128,
) -> Result<CosmosMsg, ContractError> {
    let generator_contract =
        ADOContract::default().get_cached_address(storage, ASTROPORT_GENERATOR)?;
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config
            .lp_token_address
            .expect("LP Token address not set")
            .to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: generator_contract,
            msg: to_binary(&astroport::generator::Cw20HookMsg::Deposit {})?,
            amount,
        })?,
        funds: vec![],
    }))
}

/// @dev Returns CosmosMsg struct to unstake LP Tokens from the Generator contract
/// @param lp_shares_to_unstake : LP tokens to be unstaked from generator
pub fn build_unstake_from_generator_msg(
    storage: &dyn Storage,
    config: &Config,
    lp_shares_to_withdraw: Uint128,
) -> Result<CosmosMsg, ContractError> {
    let generator_contract =
        ADOContract::default().get_cached_address(storage, ASTROPORT_GENERATOR)?;
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator_contract,
        msg: to_binary(&astroport::generator::ExecuteMsg::Withdraw {
            lp_token: config.lp_token_address.clone().expect("LP Token not set"),
            amount: lp_shares_to_withdraw,
        })?,
        funds: vec![],
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to facilitate liquidity provision to the Astroport LP Pool
/// @param slippage_tolerance : Optional slippage parameter
fn build_provide_liquidity_to_lp_pool_msg(
    deps: Deps,
    config: Config,
    state: &State,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<CosmosMsg> {
    let token = Asset {
        amount: state.total_token_deposited,
        info: AssetInfo::Token {
            contract_addr: config.token_address.clone(),
        },
    };

    let mut ust = Asset {
        amount: state.total_ust_deposited,
        info: AssetInfo::NativeToken {
            denom: String::from(UUSD_DENOM),
        },
    };

    // Deduct tax
    ust.amount = ust.amount.checked_sub(ust.compute_tax(&deps.querier)?)?;

    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config
            .astroport_lp_pool
            .expect("Token-uusd LP pool not set")
            .to_string(),
        funds: vec![Coin {
            denom: String::from(UUSD_DENOM),
            amount: ust.amount,
        }],
        msg: to_binary(&astroport::pair::ExecuteMsg::ProvideLiquidity {
            assets: [ust, token],
            slippage_tolerance,
            auto_stake: Some(false),
            receiver: None,
        })?,
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to activate MARS tokens claim from the lockdrop contract
/// @param lockdrop_contract_address : Lockdrop contract address
fn build_activate_claims_lockdrop_msg(lockdrop_contract_address: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lockdrop_contract_address.to_string(),
        msg: to_binary(&LockdropEnableClaims {})?,
        funds: vec![],
    }))
}

fn build_approve_cw20_msg(
    token_contract_address: String,
    spender_address: String,
    allowance_amount: Uint128,
) -> Result<CosmosMsg, ContractError> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address,
        msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
            spender: spender_address,
            amount: allowance_amount,
            expires: None,
        })?,
        funds: vec![],
    }))
}

fn build_transfer_cw20_token_msg(
    recipient: Addr,
    token_contract_address: String,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address,
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount,
        })?,
        funds: vec![],
    }))
}

fn build_send_cw20_token_msg(
    recipient_contract_addr: String,
    token_contract_address: String,
    amount: Uint128,
    msg_: Binary,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address,
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: recipient_contract_addr,
            amount,
            msg: msg_,
        })?,
        funds: vec![],
    }))
}
