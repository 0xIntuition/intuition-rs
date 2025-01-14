use models::{
    caip10::Caip10,
    error::ModelError,
    test_helpers::{setup_test_db, TEST_SCHEMA},
    traits::SimpleCrud,
    types::U256Wrapper,
};

#[tokio::test]
async fn test_caip10_crud() -> Result<(), ModelError> {
    let pool = setup_test_db().await;

    // Create initial CAIP10
    let id = U256Wrapper::try_from(1).unwrap();
    let caip10 = Caip10::builder()
        .id(id.clone())
        .namespace("eip155")
        .chain_id(8453)
        .account_address("0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70")
        .build()
        .upsert(&pool, TEST_SCHEMA)
        .await?;

    // Insert and verify
    let inserted = caip10.upsert(&pool, TEST_SCHEMA).await?;
    assert_eq!(inserted.namespace, "eip155");
    assert_eq!(inserted.chain_id, 8453);
    assert_eq!(
        inserted.account_address,
        "0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70"
    );

    // Update with new values
    let updated_caip10 = Caip10::builder()
        .id(id.clone())
        .namespace("eip155")
        .chain_id(1)
        .account_address("0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb71")
        .build()
        .upsert(&pool, TEST_SCHEMA)
        .await?;

    // Upsert and verify updates
    let updated = updated_caip10.upsert(&pool, TEST_SCHEMA).await?;
    assert_eq!(updated.chain_id, 1);
    assert_eq!(
        updated.account_address,
        "0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb71"
    );

    // Find by id and verify
    let found = Caip10::find_by_id(id, &pool, TEST_SCHEMA)
        .await?
        .expect("Should find CAIP10");
    assert_eq!(found.chain_id, 1);
    assert_eq!(
        found.account_address,
        "0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb71"
    );

    Ok(())
}
