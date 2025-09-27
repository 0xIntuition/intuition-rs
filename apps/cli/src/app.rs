use crate::queries::{
    GetAccountInfo, GetAccounts, GetAggregates, GetAtoms, GetPredicateObjects, GetSignals,
    get_account_info, get_accounts, get_aggregates, get_atoms, get_predicate_objects, get_signals,
};
use graphql_client::GraphQLQuery;
use std::collections::HashMap;
use std::error::Error;
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


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Aggregates,
    Accounts,
    PredicateObjects,
    Atoms,
    Signals,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Aggregates,
        Tab::Accounts,
        Tab::PredicateObjects,
        Tab::Atoms,
        Tab::Signals,
    ];

    pub fn next(self) -> Self {
        let current_index = Self::ALL.iter().position(|&t| t == self).unwrap();
        Self::ALL[(current_index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let current_index = Self::ALL.iter().position(|&t| t == self).unwrap();
        Self::ALL[(current_index + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap()
    }
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
    // Unified loading states
    loading_states: HashMap<Tab, LoadingState>,
    // GraphQL endpoint
    endpoint: String,
}

impl App {
    pub fn new(endpoint: String) -> Self {
        let mut loading_states = HashMap::new();
        for tab in Tab::ALL {
            loading_states.insert(tab, LoadingState::NotLoaded);
        }

        Self {
            current_tab: Tab::Aggregates,
            aggregates: None,
            accounts: Vec::new(),
            atoms: Vec::new(),
            signals: Vec::new(),
            predicate_objects: Vec::new(),
            selected_account: None,
            account_details: None,
            loading_states,
            endpoint,
        }
    }

    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = self.current_tab.previous();
    }

    pub async fn fetch_current_tab_data(&mut self) {
        self.set_loading_state(self.current_tab, LoadingState::Loading);

        let endpoint = self.endpoint.clone();
        let result = match self.current_tab {
            Tab::Aggregates => fetch_aggregates(&endpoint).await.map(|data| {
                self.aggregates = Some(data);
            }),
            Tab::Accounts => fetch_accounts(&endpoint).await.map(|data| {
                self.accounts = data;
            }),
            Tab::Atoms => fetch_atoms(&endpoint).await.map(|data| {
                self.atoms = data;
            }),
            Tab::Signals => fetch_signals(&endpoint).await.map(|data| {
                self.signals = data;
            }),
            Tab::PredicateObjects => fetch_predicate_objects(&endpoint).await.map(|data| {
                self.predicate_objects = data;
            }),
        };

        let state = match result {
            Ok(_) => LoadingState::Loaded,
            Err(err) => LoadingState::Error(err.to_string()),
        };
        self.set_loading_state(self.current_tab, state);
    }

    pub async fn fetch_all_data(&mut self) {
        let endpoint = self.endpoint.clone();
        for tab in Tab::ALL {
            self.set_loading_state(tab, LoadingState::Loading);

            let result = match tab {
                Tab::Aggregates => fetch_aggregates(&endpoint).await.map(|data| {
                    self.aggregates = Some(data);
                }),
                Tab::Accounts => fetch_accounts(&endpoint).await.map(|data| {
                    self.accounts = data;
                }),
                Tab::Atoms => fetch_atoms(&endpoint).await.map(|data| {
                    self.atoms = data;
                }),
                Tab::Signals => fetch_signals(&endpoint).await.map(|data| {
                    self.signals = data;
                }),
                Tab::PredicateObjects => fetch_predicate_objects(&endpoint).await.map(|data| {
                    self.predicate_objects = data;
                }),
            };

            let state = match result {
                Ok(_) => LoadingState::Loaded,
                Err(err) => LoadingState::Error(err.to_string()),
            };
            self.set_loading_state(tab, state);
        }
    }

    pub async fn initialize(&mut self) {
        // Only fetch current tab data on startup
        self.fetch_current_tab_data().await;
    }

    pub fn get_current_loading_state(&self) -> &LoadingState {
        self.loading_states.get(&self.current_tab)
            .unwrap_or(&LoadingState::NotLoaded)
    }

    pub fn get_loading_state(&self, tab: Tab) -> &LoadingState {
        self.loading_states.get(&tab)
            .unwrap_or(&LoadingState::NotLoaded)
    }

    pub fn should_load_tab(&self) -> bool {
        matches!(self.get_current_loading_state(), LoadingState::NotLoaded)
    }

    pub fn set_loading_state(&mut self, tab: Tab, state: LoadingState) {
        self.loading_states.insert(tab, state);
    }

    pub fn select_account(&mut self, id: String) {
        self.selected_account = Some(id);
    }

    pub async fn fetch_account_details(&mut self) {
        if let Some(id) = &self.selected_account {
            let details = fetch_account_info(&self.endpoint, id).await;
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

// Generic GraphQL execution function
async fn execute_graphql<T, B>(endpoint: &str, request_body: B) -> Result<T, Box<dyn Error>>
where
    T: for<'de> serde::Deserialize<'de>,
    B: serde::Serialize,
{
    let client = reqwest::Client::new();
    let res = client
        .post(endpoint)
        .json(&request_body)
        .send()
        .await?;

    let response: T = res.json().await?;
    Ok(response)
}

async fn fetch_aggregates(endpoint: &str) -> Result<get_aggregates::ResponseData, Box<dyn Error>> {
    let variables = get_aggregates::Variables {};
    let request_body = GetAggregates::build_query(variables);

    let response: graphql_client::Response<get_aggregates::ResponseData> =
        execute_graphql(endpoint, request_body).await?;

    response.data.ok_or_else(|| "No data returned from aggregates query".into())
}

async fn fetch_accounts(endpoint: &str) -> Result<Vec<get_accounts::GetAccountsAccounts>, Box<dyn Error>> {
    let variables = get_accounts::Variables {};
    let request_body = GetAccounts::build_query(variables);

    let response: graphql_client::Response<get_accounts::ResponseData> =
        execute_graphql(endpoint, request_body).await?;

    response.data
        .map(|d| d.accounts)
        .ok_or_else(|| "No data returned from accounts query".into())
}

async fn fetch_atoms(endpoint: &str) -> Result<Vec<get_atoms::GetAtomsAtoms>, Box<dyn Error>> {
    let variables = get_atoms::Variables {};
    let request_body = GetAtoms::build_query(variables);

    let response: graphql_client::Response<get_atoms::ResponseData> =
        execute_graphql(endpoint, request_body).await?;

    response.data
        .map(|d| d.atoms)
        .ok_or_else(|| "No data returned from atoms query".into())
}

async fn fetch_signals(endpoint: &str) -> Result<Vec<get_signals::GetSignalsSignals>, Box<dyn Error>> {
    let variables = get_signals::Variables {};
    let request_body = GetSignals::build_query(variables);

    let response: graphql_client::Response<get_signals::ResponseData> =
        execute_graphql(endpoint, request_body).await?;

    response.data
        .map(|d| d.signals)
        .ok_or_else(|| "No data returned from signals query".into())
}

async fn fetch_predicate_objects(endpoint: &str) -> Result<Vec<get_predicate_objects::GetPredicateObjectsPredicateObjects>, Box<dyn Error>> {
    let variables = get_predicate_objects::Variables {};
    let request_body = GetPredicateObjects::build_query(variables);

    let response: graphql_client::Response<get_predicate_objects::ResponseData> =
        execute_graphql(endpoint, request_body).await?;

    response.data
        .map(|d| d.predicate_objects)
        .ok_or_else(|| "No data returned from predicate objects query".into())
}

async fn fetch_account_info(endpoint: &str, address: &str) -> Option<get_account_info::GetAccountInfoAccount> {
    let variables = get_account_info::Variables {
        address: address.to_string(),
    };
    let request_body = GetAccountInfo::build_query(variables);

    let response: Result<graphql_client::Response<get_account_info::ResponseData>, _> =
        execute_graphql(endpoint, request_body).await;

    response.ok().and_then(|r| r.data).and_then(|d| d.account)
}