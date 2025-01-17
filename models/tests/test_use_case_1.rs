// This test is going to be removed once we have a proper testing crate
#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use models::{
        account::{Account, AccountType},
        atom::{Atom, AtomResolvingStatus, AtomType},
        atom_value::AtomValue,
        person::Person,
        test_helpers::{
            create_random_number, create_random_string, create_random_u256wrapper, setup_test_db,
            TEST_SCHEMA,
        },
        traits::SimpleCrud,
        types::U256Wrapper,
        vault::Vault,
    };

    #[tokio::test]
    async fn use_case_1() {
        let pool = setup_test_db().await;

        let mut alice_account = Account::builder()
            .id("0x000000001".to_string())
            .label("0x00...01".to_string())
            .account_type(AccountType::Default)
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store account");

        let alice_atom_wallet_account = Account::builder()
            .id("0x91".to_string())
            .label("0x91".to_string())
            .account_type(AccountType::AtomWallet)
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store account");

        let alice_vault_id = U256Wrapper::from(U256::from(1u64));

        let alice_atom = Atom::builder()
            .id(alice_vault_id.clone())
            .wallet_id(alice_atom_wallet_account.id.clone())
            .creator_id(alice_account.id.clone())
            .vault_id(alice_vault_id.clone())
            .data(alice_account.id.clone())
            .raw_data(alice_account.id.clone())
            .atom_type(AtomType::Account)
            .emoji("⛓️".to_string())
            .label("0x00...01".to_string())
            .block_number(create_random_u256wrapper())
            .block_timestamp(create_random_number())
            .transaction_hash(create_random_string())
            .resolving_status(AtomResolvingStatus::Pending)
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store atom");

        alice_account.atom_id = Some(alice_atom.id.clone());
        alice_account
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to update account");

        let _alice_vault = Vault::builder()
            .id(alice_vault_id.clone())
            .atom_id(alice_vault_id.clone())
            .total_shares(create_random_u256wrapper())
            .current_share_price(create_random_u256wrapper())
            .position_count(0)
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store vault");

        let alice_person_vault_id = U256Wrapper::from(U256::from(2u64));

        let alice_person_atom_wallet_account = Account::builder()
            .id("0x92".to_string())
            .label("0x92".to_string())
            .account_type(AccountType::AtomWallet)
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store account");

        let mut alice_person_atom = Atom::builder()
            .id(alice_person_vault_id.clone())
            .wallet_id(alice_person_atom_wallet_account.id.clone())
            .creator_id(alice_account.id.clone())
            .vault_id(alice_person_vault_id.clone())
            .data("ipfs://Qm...".to_string())
            .raw_data("ipfs://Qm...".to_string())
            .atom_type(AtomType::Person)
            .emoji("👤".to_string())
            .label("Alice".to_string())
            .block_number(create_random_u256wrapper())
            .block_timestamp(create_random_number())
            .transaction_hash(create_random_string())
            .resolving_status(AtomResolvingStatus::Pending)
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store atom");

        let _alice_person_vault = Vault::builder()
            .id(alice_person_vault_id.clone())
            .atom_id(alice_person_vault_id.clone())
            .total_shares(create_random_u256wrapper())
            .current_share_price(create_random_u256wrapper())
            .position_count(0)
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store vault");

        let alice_person = Person::builder()
            .id(alice_person_atom.id.clone())
            .name("Alice".to_string())
            .image("https://example.com/image.jpg".to_string())
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store account");

        let alice_person_atom_value = AtomValue::builder()
            .id(alice_person_atom.id.clone())
            .person_id(alice_person.id.clone())
            .build()
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to store atom value");

        alice_person_atom.value_id = Some(alice_person_atom_value.id.clone());
        alice_person_atom
            .upsert(&pool, TEST_SCHEMA)
            .await
            .expect("Failed to update atom");
    }
}
