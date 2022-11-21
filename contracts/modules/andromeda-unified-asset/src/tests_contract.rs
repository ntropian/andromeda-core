pub const LEGACY_OWNER: &str = "alice";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{instantiate, query, query_legacy_owner};
    use crate::sourced_coin::SourcedCoin;
    use crate::tests_helpers::get_test_instantiate_message;

    use andromeda_modules::unified_asset::{LegacyOwnerResponse, QueryMsg};

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coin, from_binary, Coin, Uint128};

    #[test]
    fn instantiate_and_modify_owner() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();

        let _res = instantiate(
            deps.as_mut(),
            current_env.clone(),
            mock_info(LEGACY_OWNER, &[]),
            get_test_instantiate_message(
                current_env,
                Coin {
                    amount: Uint128::from(0u128),
                    denom: "ujunox".to_string(),
                },
                false,
            ),
        )
        .unwrap();

        // ensure expected config
        let expected = LegacyOwnerResponse {
            legacy_owner: LEGACY_OWNER.to_string(),
        };
        assert_eq!(query_legacy_owner(deps.as_ref()).unwrap(), expected);

        // update owner
        // not implemented
    }

    #[test]
    fn unify_assets() {
        let mut deps = mock_dependencies();
        let current_env = mock_env();

        let _res = instantiate(
            deps.as_mut(),
            current_env.clone(),
            mock_info(LEGACY_OWNER, &[]),
            get_test_instantiate_message(
                current_env,
                Coin {
                    amount: Uint128::from(1_000_000u128),
                    denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                        .to_string(),
                },
                false,
            ),
        )
        .unwrap();

        // under test conditions, "testtokens" are worth 100 USDC each
        let test_coins: Vec<Coin> = vec![coin(10000, "testtokens"), coin(100, "testtokens")];
        let query_msg = QueryMsg::UnifyAssets {
            target_asset: Some(
                "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034".to_string(),
            ),
            assets: test_coins,
            assets_are_target_amount: false,
        };

        let res = query(deps.as_ref(), mock_env(), query_msg).unwrap();
        let res_value: SourcedCoin = from_binary(&res).unwrap();
        assert_eq!(
            res_value.coin,
            Coin {
                amount: Uint128::from(1_010_000u128),
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string()
            }
        );
    }
}
