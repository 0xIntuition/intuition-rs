#[cfg(test)]
mod tests {
    use models::{
        organization::Organization,
        test_helpers::{create_test_organization, setup_test_db},
        traits::SimpleCrud,
    };

    #[tokio::test]
    async fn test_organization_crud() {
        // Setup test database and create test atom
        let pool = setup_test_db().await;
        // Create initial organization
        let org = create_test_organization();

        // Test initial upsert
        let stored_org = org
            .upsert(&pool)
            .await
            .expect("Failed to store organization");
        assert_eq!(stored_org.name, org.name);
        assert_eq!(stored_org.description, org.description);

        // Update organization
        let mut updated_org = stored_org;
        updated_org.name = Some("Updated Test Org".to_string());
        updated_org.description = Some("An updated test organization".to_string());

        // Test update
        let stored_updated_org = updated_org
            .upsert(&pool)
            .await
            .expect("Failed to update organization");
        assert_eq!(
            stored_updated_org.name,
            Some("Updated Test Org".to_string())
        );
        assert_eq!(
            stored_updated_org.description,
            Some("An updated test organization".to_string())
        );

        // Test find_by_id
        let found_org = Organization::find_by_id(org.id.clone(), &pool)
            .await
            .expect("Failed to find organization")
            .expect("Organization not found");

        assert_eq!(found_org.id, org.id);
        assert_eq!(found_org.name, Some("Updated Test Org".to_string()));
        assert_eq!(
            found_org.description,
            Some("An updated test organization".to_string())
        );
    }
}
