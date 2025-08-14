pub mod atoms;
pub mod contracts;
pub mod triples;
pub mod types;
pub mod user;
pub mod utils;

pub use atoms::*;
pub use contracts::*;
pub use triples::*;
pub use types::*;
pub use user::*;
pub use utils::*;

use alloy::primitives::U256;
use eyre::Result;

pub struct IntuitionClient<P> {
    pub config: Config,
    pub admin_user: Option<FundedUser<P>>,
}

impl<P> IntuitionClient<P> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            admin_user: None,
        }
    }
}

impl IntuitionClient<()> {
    pub fn with_default_config() -> Self {
        Self::new(Config::default())
    }

    pub async fn initialize_admin(self) -> Result<IntuitionClient<impl alloy::providers::Provider + Clone>> {
        let admin = create_admin_user(&self.config).await?;
        Ok(IntuitionClient {
            config: self.config,
            admin_user: Some(admin),
        })
    }
}

impl<P: alloy::providers::Provider + Clone> IntuitionClient<P> {
    pub async fn get_and_fund_user(
        &self,
        user_index: u32,
        eth_amount: U256,
        token_amount: U256,
    ) -> Result<FundedUser<impl alloy::providers::Provider + Clone>> {
        let admin = self.admin_user.as_ref()
            .ok_or_else(|| eyre::eyre!("Admin user not initialized. Call initialize_admin() first."))?;
        
        get_and_fund_user(
            &self.config,
            user_index,
            eth_amount,
            token_amount,
            &admin.user.provider,
        ).await
    }

    pub async fn get_or_create_atoms<Q: alloy::providers::Provider>(
        &self,
        user: &FundedUser<Q>,
        atom_data_list: Vec<String>,
        deposit_amount: U256,
    ) -> Result<AtomCreationResult> {
        get_or_create_atoms(
            &user.contracts.multi_vault,
            atom_data_list,
            deposit_amount,
        ).await
    }

    pub async fn create_triples<Q: alloy::providers::Provider>(
        &self,
        user: &FundedUser<Q>,
        subjects: Vec<&AtomData>,
        predicates: Vec<&AtomData>,
        objects: Vec<&AtomData>,
        deposit_amount: U256,
    ) -> Result<TripleCreationResult> {
        create_triples_from_atoms(
            &user.contracts.multi_vault,
            subjects,
            predicates,
            objects,
            deposit_amount,
        ).await
    }

    pub async fn create_single_triple<Q: alloy::providers::Provider>(
        &self,
        user: &FundedUser<Q>,
        subject: &AtomData,
        predicate: &AtomData,
        object: &AtomData,
        deposit_amount: U256,
    ) -> Result<TripleCreationResult> {
        create_single_triple(
            &user.contracts.multi_vault,
            subject.id,
            predicate.id,
            object.id,
            deposit_amount,
        ).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;

    #[tokio::test]
    async fn test_intuition_client_initialization() {
        let _client = IntuitionClient::with_default_config();
        
        // Note: This test would require a running local blockchain
        // For now, it's commented out as it would fail without proper setup
        // let admin = client.initialize_admin().await.unwrap();
        // assert!(!admin.user.address.is_zero());
    }

    #[test]
    fn test_compute_atom_id() {
        use alloy::primitives::Bytes;
        
        let data = Bytes::from("test data".as_bytes().to_vec());
        let id = compute_atom_id(&data);
        
        // The ID should be deterministic for the same input
        let id2 = compute_atom_id(&data);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_wei_to_tokens() {
        let amount = U256::from(1000000000000000000u64); // 1 ETH in wei
        let tokens = wei_to_tokens(amount, 18);
        let expected = U256::from(1000000000000000000u64) * U256::from(10).pow(U256::from(18));
        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_compute_triple_id() {
        use alloy::primitives::B256;
        
        let subject_id = B256::from([1u8; 32]);
        let predicate_id = B256::from([2u8; 32]);
        let object_id = B256::from([3u8; 32]);
        
        let id = compute_triple_id(subject_id, predicate_id, object_id);
        
        // The ID should be deterministic for the same input
        let id2 = compute_triple_id(subject_id, predicate_id, object_id);
        assert_eq!(id, id2);
        
        // Different inputs should produce different IDs
        let different_id = compute_triple_id(object_id, predicate_id, subject_id);
        assert_ne!(id, different_id);
    }
}