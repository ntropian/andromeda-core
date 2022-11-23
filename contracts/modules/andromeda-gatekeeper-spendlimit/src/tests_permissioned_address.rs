#[cfg(test)]
mod tests {
    use andromeda_modules::permissioned_address::{
        CoinLimit, PeriodType, PermissionedAddress, PermissionedAddressParams,
    };
    use cosmwasm_std::Timestamp;

    use crate::constants::JUNO_MAINNET_AXLUSDC_IBC;

    #[test]
    fn permissioned_address_update_and_reset_spend_limit() {
        let starting_spend_limit = CoinLimit {
            denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 1_000_000u64,
        };
        let mut permissioned_address = PermissionedAddress::new(
            PermissionedAddressParams {
                address: "my_permissioned_address".to_string(),
                cooldown: 1510010, //seconds, meaningless here
                period_type: PeriodType::DAYS,
                period_multiple: 1,
                spend_limits: vec![starting_spend_limit.clone()],
                usdc_denom: Some("true".to_string()),
                default: Some(true),
            },
            false,
        );

        assert_eq!(
            permissioned_address
                .get_params_clone()
                .unwrap()
                .spend_limits,
            vec![starting_spend_limit.clone()]
        );

        let adjusted_spend_limit = CoinLimit {
            denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 600_000u64,
        };

        permissioned_address
            .update_spend_limit(adjusted_spend_limit.clone(), "false".to_string())
            .unwrap();
        assert_eq!(
            permissioned_address
                .get_params_clone()
                .unwrap()
                .spend_limits,
            vec![adjusted_spend_limit]
        );

        let mut permissioned_address_params = permissioned_address.get_params_clone().unwrap();
        permissioned_address_params.reset_limits();
        assert_eq!(
            permissioned_address_params.spend_limits,
            vec![starting_spend_limit]
        );

        let bigger_spend_limit = CoinLimit {
            denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
            amount: 420_000_000u64,
            limit_remaining: 420_000_000u64,
        };

        permissioned_address_params
            .update_spend_limit(bigger_spend_limit.clone())
            .unwrap();
        assert_eq!(
            permissioned_address_params.spend_limits,
            vec![bigger_spend_limit]
        );
    }

    #[test]
    fn permissioned_address_update_reset_time_period() {
        let starting_spend_limit = CoinLimit {
            denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 1_000_000u64,
        };
        let permissioned_address = PermissionedAddress::new(
            PermissionedAddressParams {
                address: "my_permissioned_address".to_string(),
                cooldown: 1_510_010, //seconds
                period_type: PeriodType::DAYS,
                period_multiple: 1,
                spend_limits: vec![starting_spend_limit.clone()],
                usdc_denom: Some("true".to_string()),
                default: Some(true),
            },
            false,
        );

        let adjusted_spend_limit = CoinLimit {
            denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
            amount: 1_000_000u64,
            limit_remaining: 600_000u64,
        };

        let mut permissioned_address_params = permissioned_address.get_params_clone().unwrap();
        permissioned_address_params.reset_limits();
        permissioned_address_params
            .update_spend_limit(adjusted_spend_limit.clone())
            .unwrap();
        assert_eq!(
            permissioned_address_params.spend_limits,
            vec![adjusted_spend_limit]
        );

        permissioned_address_params
            .reset_period(Timestamp::from_seconds(1_510_011))
            .unwrap();
        assert_eq!(
            permissioned_address_params.spend_limits,
            vec![starting_spend_limit]
        );
        assert_eq!(permissioned_address_params.cooldown, 1_510_011 + 86_400);
    }
}
