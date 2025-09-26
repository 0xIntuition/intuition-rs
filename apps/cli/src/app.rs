use crate::queries::{
    GetAccountInfo, GetAccounts, GetAggregates, GetAtoms, GetPredicateObjects, GetSignals,
    get_account_info, get_accounts, get_aggregates, get_atoms, get_predicate_objects, get_signals,
};
use graphql_client::GraphQLQuery;
use lazy_static::lazy_static;
use std::env;
use std::fmt;

#[derive(Clone, Debug)]
pub enum LoadingState {
    NotLoaded,
    Loading,
    Loaded,
    Error(String),
}

impl fmt::Display for LoadingState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadingState::NotLoaded => write!(f, "Not loaded"),
            LoadingState::Loading => write!(f, "Loading..."),
            LoadingState::Loaded => write!(f, "Loaded"),
            LoadingState::Error(msg) => write!(f, "Error: {}", msg),
        }
    }
}

lazy_static! {
    static ref GRAPHQL_ENDPOINT: String = env::var("INTUITION_URL")
        .unwrap_or_else(|_| "http://localhost:8080/v1/graphql".to_string());
}

#[derive(Clone, Copy, Debug)]
pub enum Tab {
    Aggregates,
    Accounts,
    PredicateObjects,
    Atoms,
    Signals,
}

pub struct App {
    pub current_tab: Tab,
    pub aggregates: Option<get_aggregates::ResponseData>,
    pub accounts: Vec<get_accounts::GetAccountsAccounts>,
    pub atoms: Vec<get_atoms::GetAtomsAtoms>,
    pub signals: Vec<get_signals::GetSignalsSignals>,
    pub predicate_objects: Vec<get_predicate_objects::GetPredicateObjectsPredicateObjects>,
    pub selected_account: Option<String>,
    pub account_details: Option<get_account_info::GetAccountInfoAccount>,
    // Loading states for each tab
    pub aggregates_loading: LoadingState,
    pub accounts_loading: LoadingState,
    pub atoms_loading: LoadingState,
    pub signals_loading: LoadingState,
    pub predicate_objects_loading: LoadingState,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_tab: Tab::Aggregates,
            aggregates: None,
            accounts: Vec::new(),
            atoms: Vec::new(),
            signals: Vec::new(),
            predicate_objects: Vec::new(),
            selected_account: None,
            account_details: None,
            aggregates_loading: LoadingState::NotLoaded,
            accounts_loading: LoadingState::NotLoaded,
            atoms_loading: LoadingState::NotLoaded,
            signals_loading: LoadingState::NotLoaded,
            predicate_objects_loading: LoadingState::NotLoaded,
        }
    }

    pub fn next_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Aggregates => Tab::Accounts,
            Tab::Accounts => Tab::PredicateObjects,
            Tab::PredicateObjects => Tab::Atoms,
            Tab::Atoms => Tab::Signals,
            Tab::Signals => Tab::Aggregates,
        };
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Aggregates => Tab::Signals,
            Tab::Accounts => Tab::Aggregates,
            Tab::Atoms => Tab::PredicateObjects,
            Tab::PredicateObjects => Tab::Accounts,
            Tab::Signals => Tab::Atoms,
        };
    }

    pub async fn fetch_current_tab_data(&mut self) {
        match self.current_tab {
            Tab::Aggregates => {
                self.aggregates_loading = LoadingState::Loading;
                match fetch_aggregates().await {
                    Ok(data) => {
                        self.aggregates = Some(data);
                        self.aggregates_loading = LoadingState::Loaded;
                    }
                    Err(err) => {
                        self.aggregates_loading = LoadingState::Error(err.to_string());
                    }
                }
            }
            Tab::Accounts => {
                self.accounts_loading = LoadingState::Loading;
                match fetch_accounts().await {
                    Ok(data) => {
                        self.accounts = data;
                        self.accounts_loading = LoadingState::Loaded;
                    }
                    Err(err) => {
                        self.accounts_loading = LoadingState::Error(err.to_string());
                    }
                }
            }
            Tab::Atoms => {
                self.atoms_loading = LoadingState::Loading;
                match fetch_atoms().await {
                    Ok(data) => {
                        self.atoms = data;
                        self.atoms_loading = LoadingState::Loaded;
                    }
                    Err(err) => {
                        self.atoms_loading = LoadingState::Error(err.to_string());
                    }
                }
            }
            Tab::Signals => {
                self.signals_loading = LoadingState::Loading;
                match fetch_signals().await {
                    Ok(data) => {
                        self.signals = data;
                        self.signals_loading = LoadingState::Loaded;
                    }
                    Err(err) => {
                        self.signals_loading = LoadingState::Error(err.to_string());
                    }
                }
            }
            Tab::PredicateObjects => {
                self.predicate_objects_loading = LoadingState::Loading;
                match fetch_predicate_objects().await {
                    Ok(data) => {
                        self.predicate_objects = data;
                        self.predicate_objects_loading = LoadingState::Loaded;
                    }
                    Err(err) => {
                        self.predicate_objects_loading = LoadingState::Error(err.to_string());
                    }
                }
            }
        }
    }

    pub async fn fetch_all_data(&mut self) {
        // Fetch aggregates
        self.aggregates_loading = LoadingState::Loading;
        match fetch_aggregates().await {
            Ok(data) => {
                self.aggregates = Some(data);
                self.aggregates_loading = LoadingState::Loaded;
            }
            Err(err) => {
                self.aggregates_loading = LoadingState::Error(err.to_string());
            }
        }

        // Fetch accounts
        self.accounts_loading = LoadingState::Loading;
        match fetch_accounts().await {
            Ok(data) => {
                self.accounts = data;
                self.accounts_loading = LoadingState::Loaded;
            }
            Err(err) => {
                self.accounts_loading = LoadingState::Error(err.to_string());
            }
        }

        // Fetch atoms
        self.atoms_loading = LoadingState::Loading;
        match fetch_atoms().await {
            Ok(data) => {
                self.atoms = data;
                self.atoms_loading = LoadingState::Loaded;
            }
            Err(err) => {
                self.atoms_loading = LoadingState::Error(err.to_string());
            }
        }

        // Fetch signals
        self.signals_loading = LoadingState::Loading;
        match fetch_signals().await {
            Ok(data) => {
                self.signals = data;
                self.signals_loading = LoadingState::Loaded;
            }
            Err(err) => {
                self.signals_loading = LoadingState::Error(err.to_string());
            }
        }

        // Fetch predicate objects
        self.predicate_objects_loading = LoadingState::Loading;
        match fetch_predicate_objects().await {
            Ok(data) => {
                self.predicate_objects = data;
                self.predicate_objects_loading = LoadingState::Loaded;
            }
            Err(err) => {
                self.predicate_objects_loading = LoadingState::Error(err.to_string());
            }
        }
    }

    pub async fn initialize(&mut self) {
        // Only fetch current tab data on startup
        self.fetch_current_tab_data().await;
    }

    pub fn get_current_loading_state(&self) -> &LoadingState {
        match self.current_tab {
            Tab::Aggregates => &self.aggregates_loading,
            Tab::Accounts => &self.accounts_loading,
            Tab::Atoms => &self.atoms_loading,
            Tab::Signals => &self.signals_loading,
            Tab::PredicateObjects => &self.predicate_objects_loading,
        }
    }

    pub fn should_load_tab(&self) -> bool {
        matches!(self.get_current_loading_state(), LoadingState::NotLoaded)
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

async fn fetch_aggregates() -> Result<get_aggregates::ResponseData, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let variables = get_aggregates::Variables {};
    let request_body = GetAggregates::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_aggregates::ResponseData> =
        res.json().await?;

    data.data.ok_or_else(|| "No data returned from aggregates query".into())
}

async fn fetch_accounts() -> Result<Vec<get_accounts::GetAccountsAccounts>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let variables = get_accounts::Variables {};
    let request_body = GetAccounts::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_accounts::ResponseData> =
        res.json().await?;

    data.data
        .map(|d| d.accounts)
        .ok_or_else(|| "No data returned from accounts query".into())
}

async fn fetch_atoms() -> Result<Vec<get_atoms::GetAtomsAtoms>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let variables = get_atoms::Variables {};
    let request_body = GetAtoms::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_atoms::ResponseData> =
        res.json().await?;

    data.data
        .map(|d| d.atoms)
        .ok_or_else(|| "No data returned from atoms query".into())
}

async fn fetch_signals() -> Result<Vec<get_signals::GetSignalsSignals>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let variables = get_signals::Variables {};
    let request_body = GetSignals::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_signals::ResponseData> =
        res.json().await?;

    data.data
        .map(|d| d.signals)
        .ok_or_else(|| "No data returned from signals query".into())
}

async fn fetch_predicate_objects()
-> Result<Vec<get_predicate_objects::GetPredicateObjectsPredicateObjects>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let variables = get_predicate_objects::Variables {};
    let request_body = GetPredicateObjects::build_query(variables);

    let res = client
        .post(&*GRAPHQL_ENDPOINT)
        .json(&request_body)
        .send()
        .await?;

    let data: graphql_client::Response<get_predicate_objects::ResponseData> =
        res.json().await?;

    data.data
        .map(|d| d.predicate_objects)
        .ok_or_else(|| "No data returned from predicate objects query".into())
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
        .await
        .expect("Failed to send request");

    let data: graphql_client::Response<get_account_info::ResponseData> =
        res.json().await.expect("Failed to parse response");

    data.data.and_then(|d| d.account)
}
