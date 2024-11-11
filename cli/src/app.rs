use crate::queries::{
    get_account_info, get_accounts, get_atoms,
    get_predicate_objects,
    get_signals, GetAccountInfo, GetAccounts, GetAtoms, GetPredicateObjects, GetSignals,
};
use graphql_client::GraphQLQuery;
use lazy_static::lazy_static;
use std::env;

lazy_static! {
    static ref GRAPHQL_ENDPOINT: String = env::var("INTUITION_URL")
        .unwrap_or_else(|_| "http://localhost:8080/v1/graphql".to_string());
}

pub enum Tab {
    Accounts,
    PredicateObjects,
    Atoms,
    Signals,
}

pub struct App {
    pub current_tab: Tab,
    pub accounts: Vec<get_accounts::GetAccountsAccounts>,
    pub atoms: Vec<get_atoms::GetAtomsAtoms>,
    pub signals: Vec<get_signals::GetSignalsSignals>,
    pub predicate_objects: Vec<get_predicate_objects::GetPredicateObjectsPredicateObjects>,
    pub selected_account: Option<String>,
    pub account_details: Option<get_account_info::GetAccountInfoAccount>,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_tab: Tab::Accounts,
            accounts: Vec::new(),
            atoms: Vec::new(),
            signals: Vec::new(),
            predicate_objects: Vec::new(),
            selected_account: None,
            account_details: None,
        }
    }

    pub fn next_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Accounts => Tab::PredicateObjects,
            Tab::PredicateObjects => Tab::Atoms,
            Tab::Atoms => Tab::Signals,
            Tab::Signals => Tab::Accounts,
        };
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Accounts => Tab::Signals,
            Tab::Atoms => Tab::PredicateObjects,
            Tab::PredicateObjects => Tab::Accounts,
            Tab::Signals => Tab::Atoms,
        };
    }

    pub async fn fetch_data(&mut self) {
        // Fetch accounts
        let accounts = fetch_accounts().await;
        if let Some(data) = accounts {
            self.accounts = data;
        }

        // Fetch atoms
        let atoms = fetch_atoms().await;
        if let Some(data) = atoms {
            self.atoms = data;
        }

        // Fetch signals
        let signals = fetch_signals().await;
        if let Some(data) = signals {
            self.signals = data;
        }

        // Fetch predicate objects
        let predicate_objects = fetch_predicate_objects().await;
        if let Some(data) = predicate_objects {
            self.predicate_objects = data;
        }
    }

    // Add this new method
    pub async fn initialize(&mut self) {
        self.fetch_data().await;
    }

    pub fn select_account(&mut self, id: String) {
        self.selected_account = Some(id);
    }

    pub async fn fetch_account_details(&mut self) {
        if let Some(id) = &self.selected_account {
            let details = fetch_account_info(id).await;
            self.account_details = details;
        }
    }

    pub fn next_account(&mut self) {
        if !self.accounts.is_empty() {
            let i = match self.selected_account {
                Some(ref id) => self
                    .accounts
                    .iter()
                    .position(|a| &a.id == id)
                    .map(|i| (i + 1) % self.accounts.len())
                    .unwrap_or(0),
                None => 0,
            };
            self.selected_account = Some(self.accounts[i].id.clone());
        }
    }

    pub fn previous_account(&mut self) {
        if !self.accounts.is_empty() {
            let i = match self.selected_account {
                Some(ref id) => self
                    .accounts
                    .iter()
                    .position(|a| &a.id == id)
                    .map(|i| (i + self.accounts.len() - 1) % self.accounts.len())
                    .unwrap_or(0),
                None => 0,
            };
            self.selected_account = Some(self.accounts[i].id.clone());
        }
    }

    pub fn selected_account(&self) -> Option<String> {
        self.selected_account.clone()
    }
}

async fn fetch_accounts() -> Option<Vec<get_accounts::GetAccountsAccounts>> {
    let client = reqwest::Client::new();
    let variables = get_accounts::Variables {};
    let request_body = GetAccounts::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await;

    match res {
        Ok(response) => {
            let data: Result<graphql_client::Response<get_accounts::ResponseData>, _> =
                response.json().await;
            match data {
                Ok(data) => data.data.map(|d| d.accounts),
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

async fn fetch_atoms() -> Option<Vec<get_atoms::GetAtomsAtoms>> {
    let client = reqwest::Client::new();
    let variables = get_atoms::Variables {};
    let request_body = GetAtoms::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await;

    match res {
        Ok(response) => {
            let data: Result<graphql_client::Response<get_atoms::ResponseData>, _> =
                response.json().await;
            match data {
                Ok(data) => data.data.map(|d| d.atoms),
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

async fn fetch_signals() -> Option<Vec<get_signals::GetSignalsSignals>> {
    let client = reqwest::Client::new();
    let variables = get_signals::Variables {};
    let request_body = GetSignals::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await;

    match res {
        Ok(response) => {
            let data: Result<graphql_client::Response<get_signals::ResponseData>, _> =
                response.json().await;
            match data {
                Ok(data) => data.data.map(|d| d.signals),
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

async fn fetch_predicate_objects(
) -> Option<Vec<get_predicate_objects::GetPredicateObjectsPredicateObjects>> {
    let client = reqwest::Client::new();
    let variables = get_predicate_objects::Variables {
        predicate_id: Some(4),
    };
    let request_body = GetPredicateObjects::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await;

    match res {
        Ok(response) => {
            let data: Result<graphql_client::Response<get_predicate_objects::ResponseData>, _> =
                response.json().await;
            match data {
                Ok(data) => data.data.map(|d| d.predicate_objects),
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

async fn fetch_account_info(address: &str) -> Option<get_account_info::GetAccountInfoAccount> {
    let client = reqwest::Client::new();
    let variables = get_account_info::Variables {
        address: address.to_string(),
    };
    let request_body = GetAccountInfo::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await;

    match res {
        Ok(response) => {
            let data: Result<graphql_client::Response<get_account_info::ResponseData>, _> =
                response.json().await;
            match data {
                Ok(data) => data.data.and_then(|d| d.account),
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}
