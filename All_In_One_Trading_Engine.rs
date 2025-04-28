// --- Imports and Dependencies ---
// Druid GUI and widgets 
use druid::{
    AppLauncher, Data, Lens, Widget, WidgetExt, WindowDesc, Color, LocalizedString,
    widget::{
        Flex, Label, Button, TextBox, List, Tabs, TabsPolicy, ViewSwitcher, Checkbox, RadioGroup,
        SizedBox, Scroll, Either, Container, Split, Controller, Painter, ComboBox, ProgressBar, Tooltip
    },
    AppDelegate, DelegateCtx, Handled, Selector, Command, Target,
};

// Concurrency and Async Primitives
use druid::im::Vector; // Immutable vector for app state
use std::sync::{Arc, Mutex}; // Shared state and thread safety
use chrono::{DateTime, Utc}; // Date/time utilities
use tokio::sync::mpsc; // Async multi-producer, single-consumer channels
use tokio::runtime::Runtime; // Tokio async runtime for background tasks

// Error Handling and Utilities
use thiserror::Error; // Derive macro for custom error types
use std::io; // IO error types
use std::fmt; // Formatting traits
use std::result::Result as StdResult; // Standard Result alias
use bcrypt::{hash, verify, DEFAULT_COST}; // Password hashing/verification

// Logging Frameworks
use log::{info, warn, error, debug, trace, LevelFilter}; // Logging macros
use simplelog::{
    ColorChoice, // Terminal color output
    CombinedLogger, // Combine multiple loggers
    ConfigBuilder, // Custom log config builder
    TermLogger, // Terminal logger
    TerminalMode, // Terminal output mode
    WriteLogger, // File logger
    ThreadLogMode, // Thread info in logs
    LevelPadding, // Padding for log level
    Record, // Log record struct
    Config, // Log config struct
};
use chrono::Local; // Local time for log timestamps

// Additional Concurrency, Collections, and Timing
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}; // Atomic flags for shutdown, etc.
use std::collections::HashMap; // Key-value storage
use std::time::{Duration, Instant}; // Time measurement

// File and IO 
use std::fs::File; // File operations
use std::io::Write; // For writing logs/files

// Tokio Async Utilities 
use tokio::sync::{mpsc, broadcast, oneshot, RwLock}; // Async channels and RwLock for state
use tokio::task; // Async task spawning
use tokio::time::{sleep, Duration}; // Async sleep/delay and Duration
use tokio::time::timeout; // Timeout for async ops

// Serde and HTTP 
use serde::{Deserialize}; // Serialization/deserialization
use serde_json::Value; // JSON value
use reqwest::Client; // HTTP client

// Futures
use futures::future::join_all; // For joining async futures

// IBKR Client Integration
use ibkr_client::{
    TwsClient, TwsClientConfig, TwsError,
    contract::Contract,
    order::Order as IbkrOrder,
    event::Event as IbkrEvent,
};
use ibkr_client::contract::Contract; // (redundant, but needed for some scopes)
use ibkr_client::event::Event as IbkrEvent; // (redundant, but needed for some scopes)

// Plotting 
use plotters::prelude::*; // Plotting library
use druid::widget::Painter; // Custom painting widget
use druid::{RenderContext, Size, Point, Color}; // Drawing primitives

// Editor Integration 
use druid_code_editor::{CodeEditor, EditorState, Language}; // Code editor widget

// Encryption 
use ring::aead; // Symmetric encryption
use ring::rand::{SystemRandom, SecureRandom}; // Random for nonce
use base64::{encode as b64encode, decode as b64decode}; // Base64 for key/ciphertext

// Async/Sync Utilities 
use async_trait::async_trait; // For async traits
use once_cell::sync::Lazy; // For static lazy initialization
use lru::LruCache; // LRU cache for option chain
use std::num::NonZeroUsize; // For LRU cache sizing

// IBKR State Struct

/// Centralized, thread-safe IBKR state for the app (connection, login, account, error, etc.)
#[derive(Clone, Data, Lens)]
pub struct IbkrState {
    pub is_connected: bool, // True if connected to IBKR
    pub is_logged_in: bool, // True if authenticated
    pub host: String, // IBKR host address
    pub port: u16, // IBKR port
    pub client_id: i32, // IBKR client ID
    pub error: Option<String>, // Last error message
    pub account: Option<String>, // Account code
    pub last_event: Option<String>, // Last event description
    #[data(ignore)]
    pub client: Option<Arc<Mutex<TwsClient>>>, // Thread-safe IBKR client handle
    #[data(ignore)]
    pub event_tx: Option<broadcast::Sender<IbkrEvent>>, // Channel for event-driven UI updates
}

// Editor/Widget Imports (for IDE, etc.) 
use druid::widget::{Flex, Label, Either, Controller, ControllerHost, WidgetExt, ComboBox};
use druid::{Data, Lens};

// Global AppState Arc for background threads 
use std::thread;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
static APP_STATE_ARC: Lazy<Arc<Mutex<AppState>>> = Lazy::new(|| Arc::new(Mutex::new(AppState::new())));

// Application context: Dependency injection container for services and shared state
pub struct AppContext {
    pub services: Arc<ServiceContainer>, // All DI services 
    pub user_session: Option<UserSession>, // Currently logged-in user session 
    pub config: AppConfig, // Application-wide configuration
    pub logger: Arc<dyn Logger>, // Centralized logger 
    pub shared_state: Arc<Mutex<SharedState>>, // Global mutable state (UI, background tasks, etc.)
    pub cache: Arc<Mutex<Cache>>, // Optional: cache for fast lookups
}

impl Default for IbkrState {
    fn default() -> Self {
        Self {
            is_connected: false,
            is_logged_in: false,
            host: "127.0.0.1".to_string(),
            port: 7497,
            client_id: 1,
            error: None,
            account: None,
            last_event: None,
            client: None,
            event_tx: None,
        }
    }
}

// Error Handling 

// Unified, extensible error type for the entire application (for robust error handling)
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Market data error: {0}")]
    MarketDataError(String),
    #[error("IBKR error: {0}")]
    IbkrError(String),
    #[error("Alert error: {0}")]
    AlertError(String),
    #[error("Backtest error: {0}")]
    BacktestError(String),
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    #[error("Other error: {0}")]
    Other(String),
}

// App-wide Result type alias
pub type Result<T> = StdResult<T, AppError>;

// Custom log format: timestamp, level, thread, file, line, target, message
fn advanced_log_format(
    w: &mut dyn Write,
    record: &Record<'_>,
) -> std::io::Result<()> {
    let now = Local::now();
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("unnamed");
    write!(
        w,
        "[{}][{}][{}][{}:{}][{}] {}\n",
        now.format("%Y-%m-%d %H:%M:%S%.3f"),
        record.level(),
        thread_name,
        record.file().unwrap_or("unknown"),
        record.line().unwrap_or(0),
        record.target(),
        &record.args()
    )
}

// Initialize advanced logger: logs to terminal and file, includes thread/file info
pub fn setup_logger(log_level: LevelFilter) -> Result<()> {
    let config = ConfigBuilder::new()
        .set_time_to_local(true)
        .set_thread_level(LevelFilter::Trace)
        .set_thread_mode(ThreadLogMode::Both)
        .set_level_padding(LevelPadding::Right)
        .set_target_level(LevelFilter::Trace)
        .set_location_level(LevelFilter::Trace)
        .set_format(advanced_log_format)
        .build();

    let log_file = File::create("trading_ide.log").map_err(AppError::IoError)?;

    CombinedLogger::init(vec![
        TermLogger::new(
            log_level,
            config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            log_level,
            config,
            log_file,
        ),
    ])
    .map_err(|e| AppError::Other(format!("Logger init failed: {:?}", e)))?;
    Ok(())
}

// IBKR Connection Manager: async, event-driven, auto-reconnect, error-resilient 
pub struct IbkrConnectionManager {
    pub state: Arc<RwLock<IbkrState>>, // Shared IBKR state
    pub shutdown: Arc<AtomicBool>, // Shutdown flag for graceful exit
}

impl IbkrConnectionManager {
    // Create a new connection manager with shared state
    pub fn new(state: Arc<RwLock<IbkrState>>) -> Self {
        Self {
            state,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    //  Start the connection manager in a background async task
    pub fn start(self: Arc<Self>) {
        let manager = self.clone();
        task::spawn(async move {
            manager.run().await;
        });
    }

    //  Main connection loop: handles connect, reconnect, event dispatch, error handling
    async fn run(self: Arc<Self>) {
        loop {
            if self.shutdown.load(Ordering::SeqCst) {
                info!("IBKR ConnectionManager: Shutdown requested, exiting loop.");
                break;
            }

            // Get connection parameters from state
            let (host, port, client_id) = {
                let state = self.state.read().await;
                (state.host.clone(), state.port, state.client_id)
            };

            info!("Attempting to connect to IBKR at {}:{} (client_id={})", host, port, client_id);

            let config = TwsClientConfig::default()
                .host(host.clone())
                .port(port)
                .client_id(client_id);

            match TwsClient::connect(config) {
                Ok(mut client) => {
                    let client_arc = Arc::new(Mutex::new(client));
                    {
                        let mut state = self.state.write().await;
                        state.is_connected = true;
                        state.error = None;
                        state.last_event = Some("Connected to IBKR".into());
                        state.client = Some(client_arc.clone());
                    }
                    info!("Connected to IBKR at {}:{}", host, port);

                    // Setup event channel for UI and background processing
                    let (event_tx, mut event_rx) = broadcast::channel::<IbkrEvent>(1024);
                    {
                        let mut state = self.state.write().await;
                        state.event_tx = Some(event_tx.clone());
                    }

                    // Spawn event processing task (handles incoming IBKR events)
                    let state_clone = self.state.clone();
                    let shutdown_clone = self.shutdown.clone();
                    let client_clone = client_arc.clone();
                    task::spawn_blocking(move || {
                        let mut client = client_clone.lock().unwrap();
                        for event in client.events() {
                            if shutdown_clone.load(Ordering::SeqCst) {
                                break;
                            }
                            if let Err(e) = event_tx.send(event.clone()) {
                                error!("Failed to broadcast IBKR event: {:?}", e);
                            }
                            // Handle important events for logging/state
                            match &event {
                                IbkrEvent::Error { code, message } => {
                                    error!("IBKR Error {}: {}", code, message);
                                    let mut state = futures::executor::block_on(state_clone.write());
                                    state.error = Some(format!("IBKR Error {}: {}", code, message));
                                    state.last_event = Some(format!("IBKR Error {}: {}", code, message));
                                }
                                IbkrEvent::ConnectionClosed => {
                                    warn!("IBKR connection closed by server.");
                                    let mut state = futures::executor::block_on(state_clone.write());
                                    state.is_connected = false;
                                    state.is_logged_in = false;
                                    state.last_event = Some("Connection closed by IBKR".into());
                                    break;
                                }
                                _ => {}
                            }
                        }
                    });

                    // Main event loop: receive events and update state/UI
                    while let Ok(event) = event_rx.recv().await {
                        let mut state = self.state.write().await;
                        match &event {
                            IbkrEvent::AccountUpdate { account, key, value, .. } => {
                                state.last_event = Some(format!("AccountUpdate: {} {}={}", account, key, value));
                                // Update account info, balances, etc.
                            }
                            IbkrEvent::OrderStatus { order_id, status, filled, remaining, avg_fill_price, .. } => {
                                state.last_event = Some(format!(
                                    "OrderStatus: id={} status={} filled={} remaining={} avg_price={}",
                                    order_id, status, filled, remaining, avg_fill_price
                                ));
                                // Update order book, positions, etc.
                            }
                            IbkrEvent::Execution { exec_id, symbol, side, shares, price, .. } => {
                                state.last_event = Some(format!(
                                    "Execution: id={} {} {}@{} {}",
                                    exec_id, symbol, shares, price, side
                                ));
                                // Update executions, positions, etc.
                            }
                            IbkrEvent::ConnectionClosed => {
                                state.is_connected = false;
                                state.is_logged_in = false;
                                state.last_event = Some("Connection closed by IBKR".into());
                                warn!("IBKR connection closed, will attempt reconnect.");
                                break;
                            }
                            IbkrEvent::Error { code, message } => {
                                state.error = Some(format!("IBKR Error {}: {}", code, message));
                                state.last_event = Some(format!("IBKR Error {}: {}", code, message));
                                error!("IBKR Error {}: {}", code, message);
                            }
                            _ => {
                                // Fallback: log or update state for other events
                                state.last_event = Some(format!("IBKR Event: {:?}", event));
                            }
                        }
                    }

                    // If we reach here, connection is lost or closed, attempt reconnect after delay
                    {
                        let mut state = self.state.write().await;
                        state.is_connected = false;
                        state.is_logged_in = false;
                        state.client = None;
                        state.event_tx = None;
                        state.last_event = Some("Disconnected from IBKR, will attempt reconnect".into());
                    }
                    warn!("Disconnected from IBKR, retrying in 5 seconds...");
                    sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    {
                        let mut state = self.state.write().await;
                        state.is_connected = false;
                        state.error = Some(format!("IBKR connect error: {:?}", e));
                        state.last_event = Some(format!("IBKR connect error: {:?}", e));
                    }
                    error!("Failed to connect to IBKR: {:?}, retrying in 10 seconds...", e);
                    sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    // Request shutdown of the connection manager
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

// Encryption 

// Simple symmetric encryptor for secure storage (key from env)
pub struct Encryptor {
    key: Vec<u8>, // Symmetric key bytes
}

impl Encryptor {
    // Create Encryptor from environment variable (base64 key)
    pub fn new_from_env() -> Result<Self> {
        let key = std::env::var("TRADING_IDE_KEY")
            .map_err(|_| AppError::EncryptionError("Missing encryption key in env".into()))?;
        let key_bytes = b64decode(&key).map_err(|_| AppError::EncryptionError("Invalid base64 key".into()))?;
        Ok(Self { key: key_bytes })
    }
    /// Encrypt plaintext bytes, return base64 ciphertext
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let sealing_key = aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, &self.key)
            .map_err(|_| AppError::EncryptionError("Invalid key".into()))?);
        let mut nonce = [0u8; 12];
        SystemRandom::new().fill(&mut nonce).map_err(|_| AppError::EncryptionError("Random nonce failed".into()))?;
        let mut in_out = plaintext.to_vec();
        in_out.extend_from_slice(&[0u8; aead::AES_256_GCM.tag_len()]);
        sealing_key.seal_in_place_append_tag(aead::Nonce::assume_unique_for_key(nonce), aead::Aad::empty(), &mut in_out)
            .map_err(|_| AppError::EncryptionError("Encryption failed".into()))?;
        let mut result = nonce.to_vec();
        result.extend_from_slice(&in_out);
        Ok(b64encode(&result))
    }
    // Decrypt base64 ciphertext, return plaintext bytes
    pub fn decrypt(&self, ciphertext: &str) -> Result<Vec<u8>> {
        let data = b64decode(ciphertext).map_err(|_| AppError::EncryptionError("Base64 decode failed".into()))?;
        if data.len() < 12 { return Err(AppError::EncryptionError("Ciphertext too short".into())); }
        let (nonce, mut in_out) = data.split_at(12);
        let opening_key = aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, &self.key)
            .map_err(|_| AppError::EncryptionError("Invalid key".into()))?);
        let plain = opening_key.open_in_place(aead::Nonce::try_assume_unique_for_key(nonce).unwrap(), aead::Aad::empty(), &mut in_out.to_vec())
            .map_err(|_| AppError::EncryptionError("Decryption failed".into()))?;
        Ok(plain.to_vec())
    }
}

// Database Integration (diesel, SQLite)
#[macro_use]
extern crate diesel;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

// Connect to SQLite database 
pub fn establish_connection() -> Result<SqliteConnection> {
    let db_url = std::env::var("TRADING_IDE_DB").unwrap_or_else(|_| "trading_ide.db".to_string());
    SqliteConnection::establish(&db_url)
        .map_err(|e| AppError::DatabaseError(format!("DB connect error: {}", e)))
}

// Example schema for trades and positions
table! {
    trades (id) {
        id -> Integer,
        symbol -> Text,
        price -> Double,
        quantity -> Integer,
        action -> Text,
        pnl -> Double,
        date -> Text,
    }
}
table! {
    positions (id) {
        id -> Integer,
        symbol -> Text,
        quantity -> Integer,
        average_price -> Double,
        realized_pnl -> Double,
        unrealized_pnl -> Double,
        last_traded_price -> Double,
    }
}

// Data Models 

// AppState: Top-level application state (all UI and data) 
#[derive(Clone, Data, Lens)]
pub struct AppState {
    pub user: Option<User>, // Current logged-in user
    pub login: LoginState, // Login form state
    pub dashboard: DashboardState, // Dashboard tab state
    pub positions: PositionsState, // Positions tab state
    pub trades: TradesState, // Trades tab state
    pub settings: SettingsState, // Settings tab state
    pub ide: IdeState, // IDE/editor state
    pub accounts: Vector<TradingAccountState>, // List of trading accounts
    pub selected_account: usize, // Index of selected account
    pub backtesting: BacktestingState, // Backtesting tab state
    pub error: Option<String>, // Global error message
    pub alerts: Vector<Alert>, // List of alerts
    pub bots: Vector<BotState>, // List of bots
    pub orders: Vector<Order>, // List of orders
    pub option_chain: Option<OptionChain>, // Option chain data
    pub dom: Option<DomData>, // Depth of Market data
    pub t_and_s: Option<TAndSData>, // Time & Sales data
    pub sec_filings: Vector<SecFiling>, // SEC filings
    pub notification: Option<String>, // runtime notification 
    pub ibkr: IbkrState, // IBKR integration state
}

// Login form 
#[derive(Clone, Data, Lens)]
pub struct LoginState {
    pub username: String, // Username input
    pub password: String, // Password input
    pub error: Option<String>, // Error message
}

//  Dashboard state (PnL chart, market feed) 
#[derive(Clone, Data, Lens)]
pub struct DashboardState {
    pub pnl_history: Vector<PnLPoint>, // Historical PnL points for chart
    pub market_feed: Vector<MarketFeedItem>, // Live market feed items
}

// Positions tab state 
#[derive(Clone, Data, Lens)]
pub struct PositionsState {
    pub positions: Vector<Position>, // List of positions
    pub filter: String, // Symbol filter
}

// Trades tab 
#[derive(Clone, Data, Lens)]
pub struct TradesState {
    pub trades: Vector<Trade>,// List of trades
    pub filter: String,// Filter string
}

//  Settings tab  
#[derive(Clone, Data, Lens)]
pub struct SettingsState {
    pub api_key: String, // API key for external services
    pub log_level: String, // Log level (debug/info/warn/error)
    pub log_enabled: bool, // Enable/disable logging
}

// User and roles 
#[derive(Clone, Data, Lens)]
pub struct User {
    pub username: String, // Username
    pub role: UserRole, // User role (RBAC)
    pub password_hash: Option<String>, // Password hash (bcrypt)
}

#[derive(Clone, Data, PartialEq, Eq)]
pub enum UserRole {
    Admin, // Full access
    Trader, // Can trade, edit algos
    ReadOnly, // View only
    Analyst, // Can view logs, edit algos
    Unknown, // No permissions
}

// IDE editor 
#[derive(Clone, Data, Lens)]
pub struct IdeState {
    pub files: Vector<IdeFile>, // Open files in the editor
    pub current_file: usize, // Index of current file
    pub console_output: String,  // Output/print/debug
    pub is_running: bool, // Algorithm running
    pub is_paused: bool, // Algorithm paused
    pub breakpoints: Vector<usize>, // Breakpoint lines
    pub variables: Vector<VariableWatch>, // Watched variables
    pub error: Option<String>, // IDE error
    pub current_line: usize, // Debugger: current line
}

// File in IDE 
#[derive(Clone, Data, Lens)]
pub struct IdeFile {
    pub name: String, // File name
    pub content: String, // File content
    pub path: Option<String>, // File path (if saved)
    pub is_dirty: bool, // Unsaved changes
    pub error_lines: Vector<usize>, // Error line numbers
    pub language: String, // Syntax highlighting language
    pub editor_state: druid_code_editor::EditorState, // Editor state (cursor, etc.)
}

// Variable watch (debugger)
#[derive(Clone, Data, Lens)]
pub struct VariableWatch {
    pub name: String, // Variable name
    pub value: String, // Current value
}

impl Default for IdeState {
    fn default() -> Self {
        Self {
            files: Vector::new(),
            current_file: 0,
            console_output: "".into(),
            is_running: false,
            is_paused: false,
            breakpoints: Vector::new(),
            variables: Vector::new(),
            error: None,
            current_line: 0,
        }
    }
}

// Trading accounts  
#[derive(Clone, Data, Lens)]
pub struct TradingAccountState {
    pub name: String, // Account name
    pub strategy: String, // Strategy name
    pub execution_rules: String, // Execution rules
    pub positions: Vector<Position>,  // Positions in this account
    pub trades: Vector<Trade>, // Trades in this account
    pub balance: f64, // Account balance
    pub is_active: bool, // Is account active
}

impl Default for TradingAccountState {
    fn default() -> Self {
        Self {
            name: "Default Account".into(),
            strategy: "None".into(),
            execution_rules: "".into(),
            positions: Vector::new(),
            trades: Vector::new(),
            balance: 100_000.0,
            is_active: true,
        }
    }
}

// Backtesting Engine 

#[derive(Clone, Data, Lens)]
pub struct BacktestingState {
    pub is_running: bool, // Is backtest running
    pub progress: f64, // Progress (0.0-1.0)
    pub selected_algorithm: Option<String>, // Selected algorithm name
    pub selected_account: usize, // Selected account index
    pub start_date: String, // Backtest start date
    pub end_date: String, // Backtest end date
    pub result: Option<BacktestResult>, // Backtest result
    pub error: Option<String>, // Error message
    pub selected_strategy: String, // Selected strategy
}

impl Default for BacktestingState {
    fn default() -> Self {
        Self {
            is_running: false,
            progress: 0.0,
            selected_algorithm: None,
            selected_account: 0,
            start_date: "".into(),
            end_date: "".into(),
            result: None,
            error: None,
            selected_strategy: "strategy".into(),
        }
    }
}

// Backtest result 
#[derive(Clone, Data, Lens)]
pub struct BacktestResult {
    pub pnl_history: Vector<PnLPoint>, // PnL points for chart
    pub trades: Vector<Trade>, // Trades executed
    pub summary: String, // Summary string
    pub sharpe_ratio: Option<f64>, // Sharpe ratio
    pub max_drawdown: Option<f64>, // Max drawdown
}

// Alert, Bot, Order, OptionChain, DOM, T&S, SEC Filing 
#[derive(Clone, Data, Lens)]
pub struct Alert {
    pub id: usize,  // Alert ID
    pub symbol: String, // Symbol
    pub condition: String, // Trigger condition
    pub triggered: bool, // Was triggered
    pub last_triggered: Option<String>, // Last triggered time
}

#[derive(Clone, Data, Lens)]
pub struct BotState {
    pub name: String, // Bot name
    pub is_running: bool, // Is bot running
    pub strategy: String, // Strategy name
    pub log: String, // Bot log
    pub config: String, // Bot config
}

#[derive(Clone, Data, Lens)]
pub struct Order {
    pub id: usize, // Order ID
    pub symbol: String, // Symbol
    pub price: f64, // Price
    pub quantity: i32, // Quantity
    pub status: String, // Status
    pub order_type: String, // Order type
    pub date: String, // Date
}

#[derive(Clone, Data, Lens)]
pub struct OptionChain {
    pub symbol: String, // Underlying symbol
    pub expirations: Vector<String>, // Expiration dates
    pub strikes: Vector<f64>, // Strike prices
    pub calls: Vector<OptionContract>, // Call contracts
    pub puts: Vector<OptionContract>, // Put contracts
}

#[derive(Clone, Data, Lens)]
pub struct OptionContract {
    pub strike: f64, // Strike price
    pub expiration: String, // Expiration date
    pub bid: f64, // Bid price
    pub ask: f64,// Ask price
    pub iv: f64, // Implied volatility
    pub volume: i32, // Volume
    pub open_interest: i32, // Open interest
    pub contract_type: String, // "call" or "put"
}

// Advanced Option Chain Fetching System for IBKR and Yahoo Finance

static OPTION_CHAIN_CACHE: Lazy<RwLock<LruCache<String, OptionChain>>> = Lazy::new(|| {
    RwLock::new(LruCache::new(NonZeroUsize::new(128).unwrap()))
});

#[derive(Clone, Data, Lens, PartialEq, Debug)]
pub enum OptionChainSource {
    IBKR,
    YahooFinance,
}

#[derive(Debug, Clone)]
pub enum OptionChainError {
    Network(String),
    Provider(String),
    NotFound,
    Parse(String),
    Other(String),
}

impl std::fmt::Display for OptionChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionChainError::Network(e) => write!(f, "Network error: {}", e),
            OptionChainError::Provider(e) => write!(f, "Provider error: {}", e),
            OptionChainError::NotFound => write!(f, "Option chain not found"),
            OptionChainError::Parse(e) => write!(f, "Parse error: {}", e),
            OptionChainError::Other(e) => write!(f, "Other error: {}", e),
        }
    }
}

impl std::error::Error for OptionChainError {}

#[async_trait]
pub trait OptionChainProvider: Send + Sync {
    async fn fetch_option_chain(&self, symbol: &str) -> Result<OptionChain, OptionChainError>;
    fn name(&self) -> &'static str;
}

// IBKR Option Chain Provider 
pub struct IbkrOptionChainProvider {
    pub client: Arc<Mutex<TwsClient>>,
}

#[async_trait]
impl OptionChainProvider for IbkrOptionChainProvider {
    async fn fetch_option_chain(&self, symbol: &str) -> Result<OptionChain, OptionChainError> {
        let mut client = self.client.lock().unwrap();
        let contract = Contract::stock(symbol);

        // Request option chain from IBKR
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);
        let req_id = client.req_sec_def_opt_params(&contract, move |event| {
            let _ = tx.blocking_send(event.clone());
        }).map_err(|e| OptionChainError::Provider(format!("IBKR req_sec_def_opt_params failed: {:?}", e)))?;

        // Wait for response with timeout
        let mut expirations = Vec::new();
        let mut strikes = Vec::new();
        let mut calls = Vec::new();
        let mut puts = Vec::new();

        let timeout_duration = Duration::from_secs(10);
        while let Ok(Some(event)) = timeout(timeout_duration, rx.recv()).await {
            match event {
                IbkrEvent::SecurityDefinitionOptionParameter { exchange: _, underlying_con_id: _, trading_class: _, multiplier: _, expirations: exps, strikes: strs } => {
                    expirations = exps.into_iter().collect();
                    strikes = strs.into_iter().collect();
                }
                IbkrEvent::SecurityDefinitionOptionParameterEnd { .. } => {
                    break;
                }
                _ => {}
            }
        }

        if expirations.is_empty() || strikes.is_empty() {
            return Err(OptionChainError::NotFound);
        }

        // Batch option market data requests, throttle to respect IBKR rate limits, and handle errors robustly.
        // IBKR recommends no more than ~50 market data requests per second.
        const MAX_BATCH_SIZE: usize = 25;
        const BATCH_DELAY_MS: u64 = 1200; // 1.2s between batches

        let mut option_requests = Vec::new();

        for expiration in &expirations {
            for &strike in &strikes {
                for &contract_type in &["C", "P"] {
                    let is_call = contract_type == "C";
                    let option_contract = Contract::option(symbol, expiration, strike, is_call);
                    let symbol = symbol.to_string();
                    let expiration = expiration.clone();

                    // Clone for async move
                    let mut client = self.client.lock().unwrap().clone();

                    option_requests.push(async move {
                        let (tx, mut rx) = tokio::sync::mpsc::channel(8);

                        // Request market data from IBKR
                        if let Err(e) = client.req_market_data(&option_contract, move |event| {
                            let _ = tx.blocking_send(event.clone());
                        }) {
                            log::warn!(
                                "Failed to request market data for {} {} {} {}: {:?}",
                                symbol, expiration, strike, if is_call { "Call" } else { "Put" }, e
                            );
                            return None;
                        }

                        // Await market data with timeout
                        match timeout(Duration::from_secs(2), rx.recv()).await {
                            Ok(Some(IbkrEvent::TickOptionComputation { implied_vol, bid, ask, .. })) => {
                                Some((
                                    is_call,
                                    OptionContract {
                                        strike,
                                        expiration: expiration.clone(),
                                        bid: bid.unwrap_or(0.0),
                                        ask: ask.unwrap_or(0.0),
                                        iv: implied_vol.unwrap_or(0.0),
                                        volume: 0,
                                        open_interest: 0,
                                        contract_type: if is_call { "call".to_string() } else { "put".to_string() },
                                    }
                                ))
                            }
                            Ok(Some(_)) => {
                                None
                            }
                            Ok(None) => {
                                log::debug!(
                                    "No market data received for {} {} {} {}",
                                    symbol, expiration, strike, if is_call { "Call" } else { "Put" }
                                );
                                None
                            }
                            Err(_) => {
                                log::warn!(
                                    "Timeout while waiting for market data for {} {} {} {}",
                                    symbol, expiration, strike, if is_call { "Call" } else { "Put" }
                                );
                                None
                            }
                        }
                    });
                }
            }
        }

        let mut calls = Vec::new();
        let mut puts = Vec::new();

        // Batch the requests to respect IBKR rate limits
        for batch in option_requests.chunks(MAX_BATCH_SIZE) {
            let results = join_all(batch.iter().map(|fut| fut)).await;
            for res in results {
                if let Some((is_call, option)) = res {
                    if is_call {
                        calls.push(option);
                    } else {
                        puts.push(option);
                    }
                }
            }
            // Throttle between batches
            sleep(Duration::from_millis(BATCH_DELAY_MS)).await;
        }

// Yahoo Finance Option Chain Provider 
pub struct YahooOptionChainProvider {
    pub client: reqwest::Client,
}

#[async_trait]
impl OptionChainProvider for YahooOptionChainProvider {
    async fn fetch_option_chain(&self, symbol: &str) -> Result<OptionChain, OptionChainError> {
        let url = format!("https://query2.finance.yahoo.com/v7/finance/options/{}", symbol);

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| OptionChainError::Network(format!("Yahoo request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(OptionChainError::Provider(format!(
                "Yahoo Finance API error: {} - {}",
                status, body
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| OptionChainError::Parse(format!("Yahoo JSON parse error: {}", e)))?;

        let option_chain = parse_yahoo_option_chain(symbol, &json)?;
        // Cache the result
        {
            let mut cache = OPTION_CHAIN_CACHE.write().await;
            cache.put(symbol.to_string(), option_chain.clone());
        }
        Ok(option_chain)
    }

    fn name(&self) -> &'static str {
        "YahooFinance"
    }
}

// Option Chain Fetcher with Caching and Provider Selection 
pub struct OptionChainFetcher {
    pub providers: std::collections::HashMap<OptionChainSource, std::sync::Arc<dyn OptionChainProvider>>,
}

impl OptionChainFetcher {
    pub fn new(ibkr: std::sync::Arc<tokio::sync::Mutex<TwsClient>>, http_client: reqwest::Client) -> Self {
        let mut providers: std::collections::HashMap<OptionChainSource, std::sync::Arc<dyn OptionChainProvider>> = std::collections::HashMap::new();
        providers.insert(
            OptionChainSource::IBKR,
            std::sync::Arc::new(IbkrOptionChainProvider { client: ibkr }),
        );
        providers.insert(
            OptionChainSource::YahooFinance,
            std::sync::Arc::new(YahooOptionChainProvider { client: http_client }),
        );
        Self { providers }
    }

    pub async fn fetch(
        &self,
        symbol: &str,
        source: OptionChainSource,
        use_cache: bool,
    ) -> Result<OptionChain, OptionChainError> {
        if use_cache {
            let cache = OPTION_CHAIN_CACHE.read().await;
            if let Some(chain) = cache.get(symbol) {
                return Ok(chain.clone());
            }
        }
        let provider = self.providers.get(&source)
            .ok_or_else(|| OptionChainError::Provider(format!("Provider not found: {:?}", source)))?;
        let chain = provider.fetch_option_chain(symbol).await?;
        Ok(chain)
    }
}

#[derive(Clone, Data, Lens, PartialEq, Debug)]
pub enum OptionChainSource {
    IBKR,
    YahooFinance,
}

#[derive(Debug, Clone)]
pub enum OptionChainError {
    Network(String),
    Provider(String),
    NotFound,
    Parse(String),
    Other(String),
}

impl std::fmt::Display for OptionChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionChainError::Network(e) => write!(f, "Network error: {}", e),
            OptionChainError::Provider(e) => write!(f, "Provider error: {}", e),
            OptionChainError::NotFound => write!(f, "Option chain not found"),
            OptionChainError::Parse(e) => write!(f, "Parse error: {}", e),
            OptionChainError::Other(e) => write!(f, "Other error: {}", e),
        }
    }
}

impl std::error::Error for OptionChainError {}

#[async_trait]
pub trait OptionChainProvider: Send + Sync {
    async fn fetch_option_chain(&self, symbol: &str) -> Result<OptionChain, OptionChainError>;
    fn name(&self) -> &'static str;
}

// IBKR Option Chain Provider 
pub struct IbkrOptionChainProvider {
    pub client: std::sync::Arc<tokio::sync::Mutex<TwsClient>>,
}

#[async_trait]
impl OptionChainProvider for IbkrOptionChainProvider {
    async fn fetch_option_chain(&self, symbol: &str) -> Result<OptionChain, OptionChainError> {
        // IBKR API Integration 

        // Ensure connection to IBKR TWS/Gateway
        let client = self.client.clone();
        let mut client = client.lock().await;
        if !client.is_connected().await {
            client.connect().await.map_err(|e| OptionChainError::Provider(format!("IBKR connect error: {}", e)))?;
        }

        // Request contract details for the symbol 
        let contract_details = client
            .req_contract_details(symbol)
            .await
            .map_err(|e| OptionChainError::Provider(format!("Contract details error: {}", e)))?;

        let underlying_contract = contract_details
            .get(0)
            .ok_or_else(|| OptionChainError::NotFound)?;

        // Request option chain parameters (strikes, expirations, exchanges, etc.)
        let opt_params = client
            .req_sec_def_opt_params(
                &underlying_contract.symbol,
                &underlying_contract.sec_type,
                &underlying_contract.exchange,
                underlying_contract.con_id,
            )
            .await
            .map_err(|e| OptionChainError::Provider(format!("Option params error: {}", e)))?;

        // For each expiration/strike, build option contracts and request market data
        let mut options = Vec::new();
        for expiry in &opt_params.expirations {
            for strike in &opt_params.strikes {
                for right in &["C", "P"] {
                    let option_contract = IbkrOptionContract {
                        symbol: underlying_contract.symbol.clone(),
                        expiry: expiry.clone(),
                        strike: *strike,
                        right: right.to_string(),
                        exchange: opt_params.exchange.clone(),
                        currency: underlying_contract.currency.clone(),
                    };
                    options.push(option_contract);
                }
            }
        }

        // Request market data for all option contracts (may need to batch due to IBKR rate limits)
        let mut option_quotes = std::collections::HashMap::new();
        for option in &options {
            let quote = client
                .req_market_data(option)
                .await
                .map_err(|e| OptionChainError::Provider(format!("Market data error: {}", e)))?;
            option_quotes.insert(option.clone(), quote);
        }

        // Parse and assemble OptionChain struct
        let option_chain = OptionChain::from_ibkr(
            &underlying_contract,
            &opt_params,
            &option_quotes,
        );

        Ok(option_chain)
    }
    fn name(&self) -> &'static str { "IBKR" }
}

// Yahoo Finance JSON Parsing Helper 
fn parse_yahoo_option_chain(symbol: &str, json: &serde_json::Value) -> Result<OptionChain, OptionChainError> {
    let result = json.get("optionChain")
        .and_then(|v| v.get("result"))
        .and_then(|v| v.get(0))
        .ok_or_else(|| OptionChainError::Parse("Missing optionChain.result".to_string()))?;

    let expirations = result.get("expirationDates")
        .and_then(|v| v.as_array())
        .ok_or_else(|| OptionChainError::Parse("Missing expirationDates".to_string()))?
        .iter()
        .filter_map(|v| v.as_i64())
        .map(|ts| {
            // Convert UNIX timestamp to YYYY-MM-DD
            let dt = chrono::NaiveDateTime::from_timestamp_opt(ts, 0)
                .unwrap_or_else(|| chrono::NaiveDateTime::from_timestamp(0, 0));
            dt.date().to_string()
        })
        .collect::<Vec<_>>();

    let options = result.get("options")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.get(0))
        .ok_or_else(|| OptionChainError::Parse("Missing options[0]".to_string()))?;

    let parse_contracts = |key: &str, contract_type: &str| -> Vector<OptionContract> {
        options.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().filter_map(|c| {
                    Some(OptionContract {
                        strike: c.get("strike")?.as_f64()?,
                        expiration: c.get("expiration")?.as_i64().map(|ts| {
                            let dt = chrono::NaiveDateTime::from_timestamp_opt(ts, 0)
                                .unwrap_or_else(|| chrono::NaiveDateTime::from_timestamp(0, 0));
                            dt.date().to_string()
                        })?,
                        bid: c.get("bid")?.as_f64().unwrap_or(0.0),
                        ask: c.get("ask")?.as_f64().unwrap_or(0.0),
                        iv: c.get("impliedVolatility")?.as_f64().unwrap_or(0.0),
                        volume: c.get("volume")?.as_i64().unwrap_or(0) as i32,
                        open_interest: c.get("openInterest")?.as_i64().unwrap_or(0) as i32,
                        contract_type: contract_type.to_string(),
                    })
                }).collect()
            }).unwrap_or_else(Vector::new)
    };

    let calls = parse_contracts("calls", "call");
    let puts = parse_contracts("puts", "put");

    let strikes = calls.iter().map(|c| c.strike)
        .chain(puts.iter().map(|p| p.strike))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter().collect();

    Ok(OptionChain {
        symbol: symbol.to_string(),
        expirations: Vector::from(expirations),
        strikes,
        calls,
        puts,
    })
}

// Option Chain Provider Registry and Caching System    

type ProviderArc = Arc<dyn OptionChainProvider>;

static PROVIDERS: Lazy<HashMap<OptionChainSource, ProviderArc>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(OptionChainSource::IBKR, Arc::new(IbkrOptionChainProvider {}) as ProviderArc);
    m.insert(OptionChainSource::YahooFinance, Arc::new(YahooOptionChainProvider {}) as ProviderArc);
    m
});

// Option chain cache: symbol+source -> OptionChain
static OPTION_CHAIN_CACHE: Lazy<RwLock<HashMap<(String, OptionChainSource), OptionChain>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
pub async fn fetch_option_chain_advanced(symbol: &str, source: OptionChainSource) -> Result<OptionChain, OptionChainError> {
    let cache_key = (symbol.to_uppercase(), source.clone());
    {
        let cache = OPTION_CHAIN_CACHE.read().await;
        if let Some(chain) = cache.get(&cache_key) {
            return Ok(chain.clone());
        }
    }
    let provider = PROVIDERS.get(&source)
        .ok_or_else(|| OptionChainError::Provider(format!("No provider for {:?}", source)))?;
    let chain = provider.fetch_option_chain(symbol).await?;
    {
        let mut cache = OPTION_CHAIN_CACHE.write().await;
        cache.insert(cache_key, chain.clone());
    }
    Ok(chain)
}
fn fetch_option_chain(symbol: &str) -> OptionChain {
    // Default to IBKR as the source for this sync wrapper
    let source = OptionChainSource::IBKR;
    // Use a Tokio runtime to block on the async fetch
    let rt = tokio::runtime::Handle::try_current()
            .map(|h| h.clone())
            .unwrap_or_else(|_| tokio::runtime::Runtime::new().unwrap().handle().clone());
        rt.block_on(async {
            match fetch_option_chain_advanced(symbol, source).await {
                Ok(chain) => chain,
                Err(e) => {
                    log::error!("Failed to fetch option chain for {}: {}", symbol, e);
                    // Return an empty OptionChain on error
                    OptionChain {
                        symbol: symbol.to_string(),
                        expirations: Vector::new(),
                        strikes: Vector::new(),
                        calls: Vector::new(),
                        puts: Vector::new(),
                    }
                }
            }
        })
    }

// Option Chain UI Integration

use druid::ExtEventSink;
use tokio::runtime::Handle;
use std::sync::Arc;

// OptionChainFetchRequest: Encapsulates a request to fetch an option chain.
#[derive(Clone, Debug)]
pub struct OptionChainFetchRequest {
    pub symbol: String,
    pub source: OptionChainSource,
}

// OptionChainFetchResult: Encapsulates the result (success or error) of an option chain fetch.
#[derive(Clone, Debug)]
pub struct OptionChainFetchResult {
    pub symbol: String,
    pub source: OptionChainSource,
    pub result: Result<OptionChain, OptionChainError>,
}

// OptionChainService: Centralized async service for fetching option chains and updating the UI.
pub struct OptionChainService {
    // Dependency injection: providers for each source (e.g., IBKR, YahooFinance, etc.)
    pub providers: Arc<HashMap<OptionChainSource, Arc<dyn OptionChainProvider>>>,
    pub providers: Arc<HashMap<OptionChainSource, Arc<dyn OptionChainProvider>>>,
}

impl OptionChainService {
    pub fn new(providers: HashMap<OptionChainSource, Arc<dyn OptionChainProvider>>) -> Self {
        Self {
            providers: Arc::new(providers),
        }
    }

    // Asynchronously fetches an option chain and notifies the UI via event_sink
    pub fn fetch_option_chain_and_notify(
        self: Arc<Self>,
        request: OptionChainFetchRequest,
        event_sink: ExtEventSink,
    ) {
        let providers = self.providers.clone();
        Handle::current().spawn(async move {
            let provider = match providers.get(&request.source) {
                Some(p) => p.clone(),
                None => {
                    let result = OptionChainFetchResult {
                        symbol: request.symbol.clone(),
                        source: request.source.clone(),
                        result: Err(OptionChainError::Provider(format!(
                            "No provider for source: {:?}",
                            request.source
                        ))),
                    };
                    let _ = event_sink.submit_command(
                        crate::OPTION_CHAIN_FETCHED,
                        result,
                        druid::Target::Auto,
                    );
                    return;
                }
            };

            let fetch_result = provider.fetch_option_chain(&request.symbol).await;
            let result = OptionChainFetchResult {
                symbol: request.symbol.clone(),
                source: request.source.clone(),
                result: fetch_result,
            };

            let _ = event_sink.submit_command(
                crate::OPTION_CHAIN_FETCHED,
                result,
                druid::Target::Auto,
            );
        });
    }
}

// Provider Registry Setup 

pub fn init_option_chain_service(app_state: &AppState) -> Arc<OptionChainService> {
    let mut providers: HashMap<OptionChainSource, Arc<dyn OptionChainProvider>> = HashMap::new();

    // IBKR provider (requires IBKR client from AppState)
    if let Some(client) = app_state.ibkr_state.client.clone() {
        providers.insert(
            OptionChainSource::IBKR,
            Arc::new(IbkrOptionChainProvider { client }) as Arc<dyn OptionChainProvider>,
        );
    }

    // Yahoo Finance provider
    providers.insert(
        OptionChainSource::YahooFinance,
        Arc::new(YahooFinanceOptionChainProvider::default()) as Arc<dyn OptionChainProvider>,
    );

    Arc::new(OptionChainService::new(providers))
}
use once_cell::sync::Lazy;
use std::collections::HashMap;

pub static PROVIDER_REGISTRY: Lazy<Arc<OptionChainService>> = Lazy::new(|| {
    let mut providers: HashMap<OptionChainSource, Arc<dyn OptionChainProvider>> = HashMap::new();

    // IBKR provider (requires IBKR client from AppState)
    if let Some(client) = APP_STATE_ARC.lock().unwrap().ibkr_state.client.clone() {
        providers.insert(
            OptionChainSource::IBKR,
            Arc::new(IbkrOptionChainProvider { client }) as Arc<dyn OptionChainProvider>,
        );
    }

    // Yahoo Finance provider
    providers.insert(
        OptionChainSource::YahooFinance,
        Arc::new(YahooFinanceOptionChainProvider::default()) as Arc<dyn OptionChainProvider>,
    );

    Arc::new(OptionChainService::new(providers))
});
 
#[derive(Default)]
pub struct YahooFinanceOptionChainProvider {
    pub client: reqwest::Client,
}

impl YahooFinanceOptionChainProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl OptionChainProvider for YahooFinanceOptionChainProvider {
    async fn fetch_option_chain(&self, symbol: &str) -> Result<OptionChain, OptionChainError> {
        // Build Yahoo Finance API URL
        let url = format!(
            "https://query2.finance.yahoo.com/v7/finance/options/{}",
            symbol
        );

        // Perform HTTP GET request
        let resp = self.client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (compatible; TradingEngine/1.0)")
            .send()
            .await
            .map_err(|e| OptionChainError::Network(format!("HTTP error: {}", e)))?;

        if !resp.status().is_success() {
            return Err(OptionChainError::Network(format!(
                "Yahoo Finance API returned status {}",
                resp.status()
            )));
        }

        let json: serde_json::Value = resp.json().await
            .map_err(|e| OptionChainError::Parse(format!("JSON parse error: {}", e)))?;

        // Parse the option chain data
        let result = json.get("optionChain")
            .and_then(|v| v.get("result"))
            .and_then(|v| v.get(0))
            .ok_or_else(|| OptionChainError::Parse("Malformed Yahoo Finance response".to_string()))?;

        let underlying_symbol = result.get("underlyingSymbol")
            .and_then(|v| v.as_str())
            .unwrap_or(symbol)
            .to_string();

        let expirations = result.get("expirationDates")
            .and_then(|v| v.as_array())
            .ok_or_else(|| OptionChainError::Parse("Missing expirationDates".to_string()))?
            .iter()
            .filter_map(|v| v.as_i64())
            .map(|ts| {
                // Convert UNIX timestamp to YYYY-MM-DD
                let dt = chrono::NaiveDateTime::from_timestamp_opt(ts, 0)
                    .unwrap_or_else(|| chrono::NaiveDateTime::from_timestamp(0, 0));
                dt.date().to_string()
            })
            .collect::<im::Vector<_>>();

        let mut strikes_set = std::collections::BTreeSet::new();
        let mut calls = im::Vector::new();
        let mut puts = im::Vector::new();

        if let Some(options) = result.get("options").and_then(|v| v.as_array()) {
            for chain in options {
                // Calls
                if let Some(call_arr) = chain.get("calls").and_then(|v| v.as_array()) {
                    for c in call_arr {
                        if let (Some(strike), Some(expiration), Some(bid), Some(ask), Some(iv), Some(volume), Some(oi)) = (
                            c.get("strike").and_then(|v| v.as_f64()),
                            c.get("expiration").and_then(|v| v.as_i64()),
                            c.get("bid").and_then(|v| v.as_f64()),
                            c.get("ask").and_then(|v| v.as_f64()),
                            c.get("impliedVolatility").and_then(|v| v.as_f64()),
                            c.get("volume").and_then(|v| v.as_i64()),
                            c.get("openInterest").and_then(|v| v.as_i64()),
                        ) {
                            let exp_str = chrono::NaiveDateTime::from_timestamp_opt(expiration, 0)
                                .map(|dt| dt.date().to_string())
                                .unwrap_or_else(|| "".to_string());
                            strikes_set.insert(strike);
                            calls.push_back(OptionContract {
                                strike,
                                expiration: exp_str,
                                bid,
                                ask,
                                iv,
                                volume: volume as i32,
                                open_interest: oi as i32,
                                contract_type: "call".to_string(),
                            });
                        }
                    }
                }
                // Puts
                if let Some(put_arr) = chain.get("puts").and_then(|v| v.as_array()) {
                    for p in put_arr {
                        if let (Some(strike), Some(expiration), Some(bid), Some(ask), Some(iv), Some(volume), Some(oi)) = (
                            p.get("strike").and_then(|v| v.as_f64()),
                            p.get("expiration").and_then(|v| v.as_i64()),
                            p.get("bid").and_then(|v| v.as_f64()),
                            p.get("ask").and_then(|v| v.as_f64()),
                            p.get("impliedVolatility").and_then(|v| v.as_f64()),
                            p.get("volume").and_then(|v| v.as_i64()),
                            p.get("openInterest").and_then(|v| v.as_i64()),
                        ) {
                            let exp_str = chrono::NaiveDateTime::from_timestamp_opt(expiration, 0)
                                .map(|dt| dt.date().to_string())
                                .unwrap_or_else(|| "".to_string());
                            strikes_set.insert(strike);
                            puts.push_back(OptionContract {
                                strike,
                                expiration: exp_str,
                                bid,
                                ask,
                                iv,
                                volume: volume as i32,
                                open_interest: oi as i32,
                                contract_type: "put".to_string(),
                            });
                        }
                    }
                }
            }
        }

        let strikes = strikes_set.into_iter().collect::<im::Vector<_>>();

        Ok(OptionChain {
            symbol: underlying_symbol,
            expirations,
            strikes,
            calls,
            puts,
        })
    }

    fn name(&self) -> &'static str {
        "YahooFinance"
    }
}

// Option Chain UI with advanced features: async fetch, symbol search, expiration filtering, and provider selection.

fn option_chain_ui() -> impl Widget<AppState> {
    use druid::widget::{Flex, Label, TextBox, Button, Either, List, Scroll, SizedBox, ComboBox};
    use druid::{Color, Data, Lens, WidgetExt};
    use std::sync::mpsc as std_mpsc;

    // SEC Filings (Live & Integrated)
    #[derive(Clone, Data, Lens)]
    pub struct SecFiling {
        pub symbol: String, // Symbol
        pub filing_type: String, // Filing type (e.g., 10-K)
        pub date: String, // Filing date
        pub url: String, // Filing URL
        pub description: String, // Description
    }

    // SEC EDGAR API Integration

    // Fetch SEC filings for a symbol asynchronously using the SEC EDGAR API.
    pub async fn fetch_sec_filings_async(symbol: &str) -> Result<Vec<SecFiling>, String> {
        let client = Client::new();

        // Lookup CIK for the symbol
        let cik_lookup_url = "https://www.sec.gov/files/company_tickers.json";
        let cik_map: serde_json::Value = client
            .get(cik_lookup_url)
            .header("User-Agent", "TradingEngine/1.0")
            .send()
            .await
            .map_err(|e| format!("CIK lookup HTTP error: {}", e))?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("CIK lookup JSON error: {}", e))?;

        // The JSON is a map of { "0": { "cik_str": "...", "ticker": "...", ... }, ... }
        let cik = cik_map.as_object()
            .and_then(|map| {
                map.values().find_map(|v| {
                    if v.get("ticker")?.as_str()?.eq_ignore_ascii_case(symbol) {
                        v.get("cik_str").and_then(|c| {
                            if let Some(s) = c.as_str() {
                                Some(s.to_string())
                            } else if let Some(i) = c.as_i64() {
                                Some(i.to_string())
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
            });

        let cik = match cik {
            Some(cik) => cik,
            None => return Err(format!("Symbol '{}' not found in SEC CIK database", symbol)),
        };

        // Fetch filings from SEC EDGAR API
        let filings_url = format!(
            "https://data.sec.gov/submissions/CIK{:0>10}.json",
            cik
        );
        let filings_resp: serde_json::Value = client
            .get(&filings_url)
            .header("User-Agent", "TradingEngine/1.0")
            .send()
            .await
            .map_err(|e| format!("EDGAR filings HTTP error: {}", e))?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("EDGAR filings JSON error: {}", e))?;

        // Parse filings
        let filings = filings_resp
            .get("filings")
            .and_then(|f| f.get("recent"))
            .ok_or_else(|| "Malformed SEC filings response".to_string())?;

        let forms = filings.get("form").and_then(|v| v.as_array()).ok_or("No forms array")?;
        let filing_dates = filings.get("filingDate").and_then(|v| v.as_array()).ok_or("No filingDate array")?;
        let primary_docs = filings.get("primaryDocument").and_then(|v| v.as_array()).ok_or("No primaryDocument array")?;
        let descriptions = filings.get("primaryDocDescription").and_then(|v| v.as_array());

        let mut sec_filings = Vec::new();
        let count = forms.len().min(filing_dates.len()).min(primary_docs.len());

        for i in 0..count {
            let form_type = forms[i].as_str().unwrap_or("").to_string();
            let filing_date = filing_dates[i].as_str().unwrap_or("").to_string();
            let primary_document = primary_docs[i].as_str().unwrap_or("").to_string();
            let description = descriptions
                .and_then(|arr| arr.get(i))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "".to_string());

            // Build the filing URL
            let url = format!(
                "https://www.sec.gov/Archives/edgar/data/{}/{}/{}",
                cik.trim_start_matches('0'),
                filing_date.replace("-", ""),
                primary_document
            );

            sec_filings.push(SecFiling {
                symbol: symbol.to_string(),
                filing_type: form_type,
                date: filing_date,
                url,
                description,
            });
        }

        Ok(sec_filings)
    }

    // Synchronous version for environments that do not support async.
    pub fn fetch_sec_filings(symbol: &str) -> Vec<SecFiling> {
        use reqwest::blocking::Client;

        let client = Client::new();

        // Lookup CIK for the symbol
        let cik_lookup_url = "https://www.sec.gov/files/company_tickers.json";
        let cik_map: serde_json::Value = client
            .get(cik_lookup_url)
            .header("User-Agent", "TradingEngine/1.0")
            .send()
            .and_then(|r| r.json())
            .unwrap_or_else(|_| serde_json::json!({}));

        let cik = cik_map.as_object()
            .and_then(|map| {
                map.values().find_map(|v| {
                    if v.get("ticker")?.as_str()?.eq_ignore_ascii_case(symbol) {
                        v.get("cik_str").and_then(|c| {
                            if let Some(s) = c.as_str() {
                                Some(s.to_string())
                            } else if let Some(i) = c.as_i64() {
                                Some(i.to_string())
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
            });

        let cik = match cik {
            Some(cik) => cik,
            None => return vec![], // Symbol not found
        };

        // Fetch filings from SEC EDGAR API
        let filings_url = format!(
            "https://data.sec.gov/submissions/CIK{:0>10}.json",
            cik
        );
        let filings_resp: serde_json::Value = client
            .get(&filings_url)
            .header("User-Agent", "TradingEngine/1.0")
            .send()
            .and_then(|r| r.json())
            .unwrap_or_else(|_| serde_json::json!({}));

        // Parse filings
        let mut filings = Vec::new();
        if let Some(recent) = filings_resp.get("filings").and_then(|f| f.get("recent")) {
            let forms = recent.get("form").and_then(|v| v.as_array()).unwrap_or(&vec![]);
            let filing_dates = recent.get("filingDate").and_then(|v| v.as_array()).unwrap_or(&vec![]);
            let primary_docs = recent.get("primaryDocument").and_then(|v| v.as_array()).unwrap_or(&vec![]);
            let descriptions = recent.get("primaryDocDescription").and_then(|v| v.as_array()).unwrap_or(&vec![]);

            let count = forms.len().min(filing_dates.len()).min(primary_docs.len());
            for i in 0..count {
                let form_type = forms.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let filing_date = filing_dates.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let primary_document = primary_docs.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let description = descriptions.get(i).and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default();

                let url = format!(
                    "https://www.sec.gov/Archives/edgar/data/{}/{}/{}",
                    cik.trim_start_matches('0'),
                    filing_date.replace("-", ""),
                    primary_document
                );

                filings.push(SecFiling {
                    symbol: symbol.to_string(),
                    filing_type: form_type,
                    date: filing_date,
                    url,
                    description,
                });
            }
        }
        filings
    }

    // Populate from real data source: e.g., user's watchlist, portfolio, or live market feed in AppState.
    let available_symbols: Vec<String> = data.available_symbols.iter().cloned().collect();

    // Option Chain UI Architecture, optionChainPanel, symbol selection (manual entry + dropdown), expiration filtering, provider-agnostic fetch, error reporting, calls/puts display
    fn option_chain_panel() -> impl Widget<AppState> {
        use druid::widget::{Flex, Label, TextBox, Button, Either, List, Scroll, SizedBox, ComboBox};
        use druid::{Color, WidgetExt, Lens};

        // Symbol Selection Row
        let symbol_row = Flex::row()
            .with_child(Label::new("Symbol:").fix_width(60.0))
            .with_child(
                TextBox::new()
                    .with_placeholder("e.g. AAPL")
                    .lens(AppState::option_chain.then(OptionChain::symbol).or_else(|_| "".to_string()))
                    .fix_width(100.0)
            )
            .with_spacer(8.0)
            .with_child(
                ComboBox::new(|data: &AppState, _env| data.available_symbols.clone())
                    .lens(AppState::option_chain.then(OptionChain::symbol).or_else(|_| "".to_string()))
                    .fix_width(100.0)
                    .padding(2.0)
            )
            .with_spacer(8.0)
            .with_child(
                Button::new("Fetch Option Chain").on_click(|ctx, data: &mut AppState, _| {
                    let symbol = if let Some(ref oc) = data.option_chain {
                        oc.symbol.clone()
                    } else {
                        "".to_string()
                    };
                    if symbol.trim().is_empty() {
                        data.error = Some("Please enter or select a symbol to fetch option chain.".to_string());
                        return;
                    }
                    // Advanced real app architecture: async, provider-driven, with UI feedback and error handling.
                    // This uses Druid's command/event system and a background Tokio runtime for async fetches.

                    use druid::{Command, Selector, Target, ExtEventSink};
                    use std::sync::Arc;

                    // Define selectors for command-based async workflow
                    const FETCH_OPTION_CHAIN: Selector<(String, OptionChainSource)> = Selector::new("app.fetch-option-chain");
                    const OPTION_CHAIN_FETCHED: Selector<Result<OptionChain, OptionChainError>> = Selector::new("app.option-chain-fetched");

                    // When the button is clicked, submit a command to trigger async fetch.
                    // The provider can be selected by the user; here we use a field in AppState.
                    let symbol_for_fetch = symbol.clone();
                    let provider_for_fetch = data.selected_option_chain_provider.clone();
                    ctx.submit_command(
                        Command::new(
                            FETCH_OPTION_CHAIN,
                            (symbol_for_fetch, provider_for_fetch),
                            Target::Global,
                        )
                    );

                     // Async fetch logic for option chains 

                    match command.selector {
                        FETCH_OPTION_CHAIN => {
                            // Extract symbol and provider from the command payload
                            let (symbol, provider) = command.get_unchecked(FETCH_OPTION_CHAIN).clone();
                            let event_sink = ctx.get_external_handle();
                            let provider_registry = PROVIDER_REGISTRY.clone();

                            // Spawn async fetch on a background thread (Tokio runtime)
                            tokio::spawn(async move {
                                let fetch_result = provider_registry
                                    .get_provider(provider)
                                    .map(|prov| prov.fetch_option_chain(&symbol))
                                    .ok_or_else(|| OptionChainError::Provider("Provider not found".to_string()));

                                let result = match fetch_result {
                                    Ok(fut) => fut.await,
                                    Err(e) => Err(e),
                                };

                                let _ = event_sink.submit_command(
                                    OPTION_CHAIN_FETCHED,
                                    result,
                                    Target::Auto,
                                );
                            });
                            return Handled::Yes;
                        }
                        OPTION_CHAIN_FETCHED => {
                            // Update AppState with the fetched option chain 
                            let result = command.get_unchecked(OPTION_CHAIN_FETCHED).clone();
                            match result {
                                Ok(chain) => {
                                    data.option_chain = Some(chain);
                                    data.error = None;
                                }
                                Err(e) => {
                                    data.option_chain = None;
                                    data.error = Some(format!("Failed to fetch option chain: {}", e));
                                }
                            }
                            ctx.request_update();
                            return Handled::Yes;
                        _ => {}
                    }

        // Expiration Filter Row
        let expiration_filter = Either::new(
            |data: &AppState, _| {
                if let Some(ref oc) = data.option_chain {
                    !oc.expirations.is_empty()
                } else {
                    false
                }
            },
            Flex::row()
                .with_child(Label::new("Expiration:").fix_width(80.0))
                .with_child(
                    ComboBox::new(|data: &AppState, _env| {
                        if let Some(ref oc) = data.option_chain {
                            oc.expirations.iter().cloned().collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        }
                    })
                    .lens(AppState::option_chain.then(OptionChain::expirations).or_else(|_| Vector::new()))
                    .fix_width(120.0)
                ),
            SizedBox::empty(),
        );

        // Calls Table
        let calls_table = Scroll::new(
            List::new(|| {
                Flex::row()
                    .with_child(Label::new(|c: &OptionContract, _| format!("{} {}", c.contract_type, c.strike)).fix_width(80.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("Exp: {}", c.expiration)).fix_width(90.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("Bid: {:.2}", c.bid)).fix_width(70.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("Ask: {:.2}", c.ask)).fix_width(70.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("IV: {:.2}%", c.iv * 100.0)).fix_width(70.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("Vol: {}", c.volume)).fix_width(60.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("OI: {}", c.open_interest)).fix_width(60.0))
            })
            .lens(AppState::option_chain.then(OptionChain::calls))
        )
        .vertical()
        .fix_height(180.0);

        // Puts Table
        let puts_table = Scroll::new(
            List::new(|| {
                Flex::row()
                    .with_child(Label::new(|c: &OptionContract, _| format!("{} {}", c.contract_type, c.strike)).fix_width(80.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("Exp: {}", c.expiration)).fix_width(90.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("Bid: {:.2}", c.bid)).fix_width(70.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("Ask: {:.2}", c.ask)).fix_width(70.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("IV: {:.2}%", c.iv * 100.0)).fix_width(70.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("Vol: {}", c.volume)).fix_width(60.0))
                    .with_child(Label::new(|c: &OptionContract, _| format!("OI: {}", c.open_interest)).fix_width(60.0))
            })
            .lens(AppState::option_chain.then(OptionChain::puts))
        )
        .vertical()
        .fix_height(180.0);

        // Option Chain Details Section 
        let option_chain_section = Either::new(
            |data: &AppState, _| data.option_chain.is_some(),
            Flex::column()
                .with_child(Label::new(|data: &AppState, _| {
                    if let Some(ref oc) = data.option_chain {
                        format!("Option Chain for {}", oc.symbol)
                    } else {
                        "".to_string()
                    }
                }).with_text_size(18.0))
                .with_spacer(6.0)
                .with_child(expiration_filter)
                .with_spacer(6.0)
                .with_child(Label::new("Calls").with_text_size(16.0))
                .with_child(calls_table)
                .with_spacer(8.0)
                .with_child(Label::new("Puts").with_text_size(16.0))
                .with_child(puts_table),
            SizedBox::empty(),
        );

        // Error Display 
        let error_display = Either::new(
            |data: &AppState, _| data.error.is_some(),
            Label::dynamic(|data: &AppState, _| data.error.clone().unwrap_or_default())
                .with_text_color(Color::rgb8(200, 0, 0)),
            SizedBox::empty(),
        );

        // Compose Main Panel 
        Flex::column()
            .with_child(symbol_row)
            .with_spacer(10.0)
            .with_child(option_chain_section)
            .with_spacer(10.0)
            .with_child(error_display)
            .padding(10.0)
    }

    // Use the modular panel in the main UI
    option_chain_panel()

// Account Selector UI

fn account_selector_ui() -> impl Widget<AppState> {
    Flex::row()
        .with_child(Label::new("Account:").with_text_size(14.0))
        .with_spacer(4.0)
        .with_child(
            ComboBox::new(|| {
                Box::new(|data: &AppState, _env| {
                    data.accounts.iter().map(|acc| acc.name.clone()).collect::<Vec<_>>()
                })
            })
            .lens(AppState::selected_account)
        )
        .with_spacer(8.0)
        .with_child(Button::new("Add Account").on_click(|_ctx, data: &mut AppState, _| {
            // Add a new trading account
            let mut accounts = data.accounts.clone();
            accounts.push_back(TradingAccountState {
                name: format!("Account {}", accounts.len() + 1),
                ..TradingAccountState::default()
            });
            data.accounts = accounts;
            data.selected_account = data.accounts.len() - 1;
            data.notification = Some("New account added.".into());
        }).with_tooltip("Add a new trading account"))
        .with_spacer(4.0)
        .with_child(Button::new("Remove Account").on_click(|_ctx, data: &mut AppState, _| {
            // Remove selected account (if more than one)
            if data.accounts.len() > 1 {
                let idx = data.selected_account;
                let mut accounts = data.accounts.clone();
                accounts.remove(idx);
                data.accounts = accounts;
                data.selected_account = 0;
                data.notification = Some("Account removed.".into());
            } else {
                data.notification = Some("Cannot remove the last account.".into());
            }
        }).with_tooltip("Remove the selected trading account"))
        .padding((10.0, 4.0, 10.0, 4.0))
}

// Charting UI with interactive candlestick charts, trend lines, volume overlays, and technical indicators.

/// ChartData holds the OHLCV data for chart rendering.
#[derive(Clone, Data, Lens)]
pub struct ChartData {
    pub timestamps: Vector<i64>, // Unix timestamps
    pub opens: Vector<f64>,
    pub highs: Vector<f64>,
    pub lows: Vector<f64>,
    pub closes: Vector<f64>,
    pub volumes: Vector<f64>,
    pub symbol: String,
    pub indicator_lines: Vector<(String, Vector<f64>)>, // (name, values)
}

// Returns a Druid widget that renders an advanced chart for the selected symbol.
fn advanced_charting_ui() -> impl Widget<AppState> {
    Painter::new(|ctx, data: &AppState, env| {
        let chart_data = data.selected_chart_data();
        let size = ctx.size();
        let root = ctx.to_piet();
        let root_area = plotters::backend::PietDrawingArea::new(&root, (size.width as u32, size.height as u32));
        root_area.fill(&WHITE).ok();

        if let Some(chart) = chart_data {
            let x_range = chart.timestamps.iter().min().cloned().unwrap_or(0)..chart.timestamps.iter().max().cloned().unwrap_or(1);
            let y_min = chart.lows.iter().cloned().fold(f64::INFINITY, f64::min);
            let y_max = chart.highs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            let mut chart_ctx = ChartBuilder::on(&root_area)
                .margin(10)
                .caption(format!("{} - Advanced Chart", chart.symbol), ("sans-serif", 18))
                .x_label_area_size(30)
                .y_label_area_size(40)
                .build_cartesian_2d(x_range.clone(), y_min..y_max)
                .unwrap();

            chart_ctx.configure_mesh()
                .x_labels(10)
                .y_labels(8)
                .disable_mesh()
                .draw()
                .unwrap();

            // Draw candlesticks
            for i in 0..chart.timestamps.len() {
                let x = chart.timestamps[i];
                let open = chart.opens[i];
                let high = chart.highs[i];
                let low = chart.lows[i];
                let close = chart.closes[i];
                let color = if close >= open { &GREEN } else { &RED };
                chart_ctx.draw_series(std::iter::once(CandleStick::new(
                    x, open, high, low, close, color,
                ))).ok();
            }

            // Draw indicator lines (e.g., moving averages)
            for (name, values) in &chart.indicator_lines {
                let points: Vec<_> = chart.timestamps.iter().cloned().zip(values.iter().cloned()).collect();
                chart_ctx.draw_series(LineSeries::new(points, &BLUE)).ok();
            }

            // Draw volume as a secondary chart 

            // Draw volume as a secondary chart below the main price chart
            if let Some(chart) = chart_data {
                let volume_max = chart.volumes.iter().cloned().fold(0.0, f64::max);
                let volume_area_height = size.height * 0.20;
                let price_area_height = size.height * 0.75;
                let volume_area_y = price_area_height + 10.0;

                // Draw volume bars
                for (i, &x) in chart.timestamps.iter().enumerate() {
                    let volume = chart.volumes.get(i).cloned().unwrap_or(0.0);
                    let bar_width = (size.width / chart.timestamps.len() as f64).max(1.0);
                    let bar_height = if volume_max > 0.0 {
                        (volume / volume_max) * volume_area_height
                    } else {
                        0.0
                    };
                    let bar_x = i as f64 * bar_width;
                    let bar_y = volume_area_y + (volume_area_height - bar_height);

                    let bar_color = if chart.closes[i] >= chart.opens[i] { &GREEN } else { &RED };
                    ctx.fill(
                        druid::kurbo::Rect::new(bar_x, bar_y, bar_x + bar_width * 0.8, volume_area_y + volume_area_height),
                        bar_color,
                    );
                }

                // Draw trend lines (user-drawn or auto-detected)
                if let Some(trend_lines) = &chart.trend_lines {
                    for line in trend_lines {
                        let (x1, y1, x2, y2) = (line.x1, line.y1, line.x2, line.y2);
                        ctx.stroke(
                            druid::kurbo::Line::new(
                                Point::new(x1, y1),
                                Point::new(x2, y2),
                            ),
                            &Color::rgb8(255, 165, 0), // Orange for trend lines
                            2.0,
                        );
                    }
                }

                // Draw buy/sell signals (arrows or markers)
                if let Some(signals) = &chart.trade_signals {
                    for signal in signals {
                        let idx = signal.index;
                        if idx < chart.timestamps.len() {
                            let x = chart.timestamps[idx];
                            let y = if signal.signal_type == "buy" {
                                chart.lows[idx]
                            } else {
                                chart.highs[idx]
                            };
                            let color = if signal.signal_type == "buy" { &GREEN } else { &RED };
                            // Draw a small triangle as marker
                            let marker = druid::kurbo::Circle::new(Point::new(x as f64, y), 5.0);
                            ctx.fill(marker, color);
                        }
                    }
                }

                // Draw tooltips for mouse hover (if supported)
                if let Some(mouse_pos) = data.chart_mouse_position {
                    // Find nearest candle
                    if let Some((i, _)) = chart.timestamps.iter().enumerate()
                        .min_by_key(|(_, &t)| ((t as f64 - mouse_pos.x).abs() * 1000.0) as u64)
                    {
                        let tooltip_text = format!(
                            "O:{:.2} H:{:.2} L:{:.2} C:{:.2} V:{}",
                            chart.opens[i], chart.highs[i], chart.lows[i], chart.closes[i], chart.volumes[i] as u64
                        );
                        ctx.draw_text(
                            &ctx.text().new_text_layout(tooltip_text)
                                .text_color(Color::BLACK)
                                .font(druid::FontDescriptor::new(druid::FontFamily::SYSTEM_UI).with_size(12.0))
                                .build().unwrap(),
                            Point::new(mouse_pos.x + 10.0, mouse_pos.y - 20.0),
                        );
                    }
                }
            }
        } else {
            // No chart data available
            let text = "No chart data available. Select a symbol to view its chart.";
            ctx.draw_text(
                &ctx.text().new_text_layout(text).text_color(Color::grey(0.5)).font(druid::FontDescriptor::new(druid::FontFamily::SYSTEM_UI).with_size(16.0)).build().unwrap(),
                Point::new(20.0, size.height / 2.0 - 10.0),
            );
        }
    })
    .fix_height(400.0)
    .padding(10.0)
fn advanced_charting_ui() -> impl Widget<AppState> {
    Flex::column()
        .with_child(Label::new("Advanced Charting (Coming Soon)").with_text_size(18.0))
        .with_child(Label::new("Trend lines, volume indicators, and more will be available here."))
        .padding(10.0)
}

// Main App UI (Tabs)

fn main_app_ui() -> impl Widget<AppState> {
    Tabs::for_policy(MainTabs)
        .with_axis(druid::Axis::Horizontal)
        .lens(druid::lens::Identity)
}

// Tabs for main app: IDE, Dashboard, Positions, Trades, Settings, Backtesting, User Management
struct MainTabs;

impl TabsPolicy for MainTabs {
    type Key = usize;
    type Input = AppState;
    type LabelWidget = Label<AppState>;
    type BodyWidget = Box<dyn Widget<AppState>>;

    fn tabs(data: &Self::Input) -> Vec<Self::Key> {
        let mut tabs = vec![0, 1, 2, 3, 4, 5];
        if let Some(user) = &data.user {
            if user.role.can_manage_users() {
                tabs.push(6);
            }
        }
        tabs
    }
    fn tab_info(key: Self::Key, _data: &Self::Input) -> (Self::LabelWidget, Self::BodyWidget) {
        match key {
            0 => (Label::new("IDE"), Box::new(ide_ui())),
            1 => (Label::new("Dashboard"), Box::new(dashboard_ui())),
            2 => (Label::new("Positions"), Box::new(positions_ui())),
            3 => (Label::new("Trade History"), Box::new(trades_ui())),
            4 => (Label::new("Settings"), Box::new(settings_ui())),
            5 => (Label::new("Backtesting"), Box::new(backtesting_ui())),
            6 => (Label::new("User Management"), Box::new(user_management_ui())),
            _ => (Label::new("Unknown"), Box::new(Label::new("Unknown Tab"))),
        }
    }
    fn active_tab(_data: &Self::Input) -> Self::Key {
        0
    }
    fn with_label(_key: &Self::Key, label: Self::LabelWidget) -> Self::LabelWidget {
        label
    }
}

// IDE Tab (Trading Algorithm Editor)

fn ide_ui() -> impl Widget<AppState> {
    // Split: Left = File List, Center = Code Editor, Right = Variable Watch, Bottom = Console Output
    Split::columns(
        // File List and File Management
        Flex::column()
            .with_child(Label::new("Files").with_text_size(16.0))
            .with_spacer(4.0)
            .with_flex_child(file_list_ui(), 1.0)
            .with_spacer(4.0)
            .with_child(file_management_buttons()),
        // Main Editor and Debugger
        Split::rows(
            Flex::column()
                .with_child(
                    Flex::row()
                        .with_child(Label::new("Algorithm Editor").with_text_size(16.0))
                        .with_spacer(10.0)
                        .with_child(algorithm_execution_controls())
                        .with_spacer(10.0)
                        .with_child(debugger_controls())
                )
                .with_spacer(4.0)
                .with_flex_child(code_editor_ui(), 1.0),
            // Console Output
            Flex::column()
                .with_child(Label::new("Console Output").with_text_size(14.0))
                .with_spacer(2.0)
                .with_flex_child(console_output_ui(), 1.0)
                .padding(4.0),
            0.7,
        ),
        // Variable Watch
        Flex::column()
            .with_child(Label::new("Variable Watch").with_text_size(14.0))
            .with_spacer(2.0)
            .with_flex_child(variable_watch_ui(), 1.0)
            .padding(4.0),
        0.18,
    )
    .min_size((
        1200.0,// width
        700.0,// height
    ))
    .padding(8.0)
}

// File List UI

fn file_list_ui() -> impl Widget<AppState> {
    List::new(|| {
        Flex::row()
            .with_child(Label::new(|f: &IdeFile, _| f.name.clone()).fix_width(120.0))
            .with_spacer(4.0)
            .with_child(
                Either::new(
                    |f: &IdeFile, _| f.is_dirty,
                    Label::new("*").with_text_color(Color::rgb8(200, 80, 80)),
                    SizedBox::empty(),
                )
            )
    })
    .lens(AppState::ide.then(IdeState::files))
}

// RBAC Helper 

impl UserRole {
    pub fn can_manage_users(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
    pub fn can_trade(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Trader)
    }
    pub fn can_view_positions(&self) -> bool {
        !matches!(self, UserRole::Unknown)
    }
    pub fn can_view_logs(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Analyst)
    }
    pub fn can_edit_algorithms(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Trader | UserRole::Analyst)
    }
    pub fn can_manage_bots(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Trader)
    }
    pub fn can_view_dom(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Trader)
    }
}

// Code Editor UI (with syntax highlighting for multiple languages, error highlighting, breakpoints)

fn code_editor_ui() -> impl Widget<AppState> {
    // Lens to get the current file's editor state
    let editor_lens = AppState::ide
        .then(IdeState::files)
        .index(AppState::ide.then(IdeState::current_file))
        .then(IdeFile::editor_state);

    // Lens to get the current file's language (as a string)
    let language_lens = AppState::ide
        .then(IdeState::files)
        .index(AppState::ide.then(IdeState::current_file))
        .then(IdeFile::language);

    // List of supported languages for syntax highlighting
    const LANGUAGES: &[(&str, Language)] = &[
        ("C", Language::C),
        ("C++", Language::Cpp),
        ("C#", Language::CSharp),
        ("Python", Language::Python),
        ("JavaScript", Language::JavaScript),
        ("Rust", Language::Rust),
        ("Go", Language::Go),
        ("Plain Text", Language::PlainText),
    ];

    // Helper to map string to Language
    fn language_from_str(lang: &str) -> Language {
        match lang {
            "C" => Language::C,
            "C++" => Language::Cpp,
            "C#" => Language::CSharp,
            "Python" => Language::Python,
            "JavaScript" => Language::JavaScript,
            "Rust" => Language::Rust,
            "Go" => Language::Go,
            _ => Language::PlainText,
        }
    }

    // ComboBox for language selection
    let language_picker = ComboBox::new(
        LANGUAGES.iter().map(|(name, _)| name.to_string()).collect::<Vec<_>>(),
    )
    .lens(language_lens.clone())
    .on_change(|_ctx, lang: &mut String, data: &mut AppState, _env| {
        // Update the language in the current file's editor state if needed
        if let Some(current_file) = data.ide.files.get_mut(data.ide.current_file) {
            current_file.language = lang.clone();
        }
    });

    // The code editor, with dynamic language selection
    let code_editor = CodeEditor::new()
        .with_language_fn(move |data: &AppState, _env| {
            let lang = language_lens.get(data);
            language_from_str(&lang)
        })
        .with_autocomplete(true)
        .with_error_highlighting(true)
        .with_code_folding(true)
        .lens(editor_lens);

    Flex::column()
        .with_child(
            Flex::row()
                .with_child(Label::new("Language:").with_text_size(12.0))
                .with_spacer(4.0)
                .with_flex_child(language_picker, 1.0)
                .padding((0.0, 0.0, 0.0, 4.0))
        )
        .with_flex_child(code_editor, 1.0)
        .padding(4.0)
        .background(Color::grey8(0xF8))
        .border(Color::grey(0.6), 1.0)
}

// Algorithm Execution Controls (Run, Pause, Stop, Reset)

fn algorithm_execution_controls() -> impl Widget<AppState> {
    Flex::row()
        .with_child(Button::new("Run").on_click(|_ctx, data: &mut AppState, _| {
            // Start algorithm execution
            if let Some(_current_file) = data.ide.files.get(data.ide.current_file) {
                data.ide.console_output.clear();
                data.ide.is_running = true;
                data.ide.is_paused = false;
            } else {
                data.ide.console_output = "Error: No file selected.".into();
                data.ide.is_running = false;
            }
            data.ide.is_running = true;
            data.ide.is_paused = false;
            data.ide.console_output = "Running algorithm...".into();
        }))
        .with_spacer(4.0)
        .with_child(Button::new("Pause").on_click(|_ctx, data: &mut AppState, _| {
            // Pause algorithm
            if data.ide.is_running && !data.ide.is_paused {
                data.ide.is_paused = true;
                data.ide.console_output += "\nAlgorithm paused.";
            } else if !data.ide.is_running {
                data.ide.console_output += "\nCannot pause: Algorithm is not running.";
            } else if data.ide.is_paused {
                data.ide.console_output += "\nAlgorithm is already paused.";
            }
        }))
        .with_spacer(4.0)
        .with_child(Button::new("Stop").on_click(|_ctx, data: &mut AppState, _| {
            // Stop execution
            data.ide.is_running = false;
            data.ide.is_paused = false;
            data.ide.console_output += "\nAlgorithm stopped.";
        }))
        .with_spacer(4.0)
        .with_child(Button::new("Reset").on_click(|_ctx, data: &mut AppState, _| {
            // Reset environment: clear variables, breakpoints, and console output, and reset running/paused state
            data.ide.variables.clear();
            data.ide.breakpoints.clear();
            data.ide.console_output = "".into();
            data.ide.is_running = false;
            data.ide.is_paused = false;
        }))
}

// Debugger Controls (Step, Breakpoints)

fn debugger_controls() -> impl Widget<AppState> {
    Flex::row()
        .with_child(Button::new("Step").on_click(|_ctx, data: &mut AppState, _| {
            // Step through code: advance to next line if running and not paused
            if data.ide.is_running && !data.ide.is_paused {
                data.ide.current_line += 1;
                data.ide.console_output += &format!("\nStepped to line {}.", data.ide.current_line);
            } else if !data.ide.is_running {
                data.ide.console_output += "\nCannot step: Algorithm is not running.";
            } else if data.ide.is_paused {
                data.ide.console_output += "\nCannot step: Algorithm is paused.";
            }
        }))
        .with_spacer(4.0)
        .with_child(Button::new("Add Breakpoint").on_click(|_ctx, data: &mut AppState, _| {
            // Add breakpoint at current line
            data.ide.breakpoints.push_back(data.ide.current_line);
            data.ide.console_output += "\nBreakpoint added.";
        }))
}


// Console Output UI

fn console_output_ui() -> impl Widget<AppState> {
    Scroll::new(
        Label::dynamic(|data: &AppState, _| data.ide.console_output.clone())
            .with_text_size(13.0)
            .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
    )
    .vertical()
    .border(Color::grey(0.7), 1.0)
    .background(Color::grey8(0xF0))
}

// Variable Watch UI

fn variable_watch_ui() -> impl Widget<AppState> {
    List::new(|| {
        Flex::row()
            .with_child(Label::new(|v: &VariableWatch, _| v.name.clone()).fix_width(100.0))
            .with_child(Label::new(|v: &VariableWatch, _| v.value.clone()).fix_width(100.0))
    })
    .lens(AppState::ide.then(IdeState::variables))
}

// Dashboard Tab

fn dashboard_ui() -> impl Widget<AppState> {
    Flex::column()
        .with_child(Label::new("Portfolio Overview").with_text_size(24.0))
        .with_spacer(10.0)
        .with_child(
            pnl_chart()
                .lens(AppState::dashboard.then(DashboardState::pnl_history))
        )
        .with_spacer(20.0)
        .with_child(Label::new("Market Feed").with_text_size(18.0))
        .with_child(
            Scroll::new(market_feed_list())
                .vertical()
                .lens(AppState::dashboard.then(DashboardState::market_feed))
        )
        .padding(10.0)
}

// PNL Chart Widget

fn pnl_chart() -> impl Widget<Vector<PnLPoint>> {
    use druid::kurbo::Line;
    use druid::piet::{Color as PietColor, RenderContext};
    use druid::widget::prelude::*;
    use druid::{Widget, WidgetExt, Data, Lens, Size, Point, Rect};

    struct PnLChartWidget;

    impl Widget<Vector<PnLPoint>> for PnLChartWidget {
        fn event(&mut self, _ctx: &mut EventCtx, _event: &Event, _data: &mut Vector<PnLPoint>, _env: &Env) {}
        fn lifecycle(&mut self, _ctx: &mut LifeCycleCtx, _event: &LifeCycle, _data: &Vector<PnLPoint>, _env: &Env) {}
        fn update(&mut self, ctx: &mut UpdateCtx, old_data: &Vector<PnLPoint>, data: &Vector<PnLPoint>, _env: &Env) {
            if !old_data.same(data) {
                ctx.request_paint();
            }
        }
        fn layout(&mut self, _ctx: &mut LayoutCtx, bc: &BoxConstraints, _data: &Vector<PnLPoint>, _env: &Env) -> Size {
            let size = Size::new(bc.max().width.max(300.0), bc.max().height.max(120.0));
            size
        }
        fn paint(&mut self, ctx: &mut PaintCtx, data: &Vector<PnLPoint>, _env: &Env) {
            let rect = ctx.size().to_rect();
            ctx.fill(rect, &PietColor::WHITE);

            if data.is_empty() {
                let text = ctx.text();
                let layout = text
                    .new_text_layout("PnL Chart (no data)")
                    .text_color(PietColor::grey(0.5))
                    .font(druid::FontFamily::SYSTEM_UI, 16.0)
                    .build()
                    .unwrap();
                ctx.draw_text(&layout, Point::new(10.0, rect.height() / 2.0 - 10.0));
                return;
            }

            // Find min/max for scaling
            let min_x = data.first().map(|p| p.time).unwrap_or(0.0);
            let max_x = data.last().map(|p| p.time).unwrap_or(1.0);
            let (min_y, max_y) = data.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), p| {
                (min.min(p.pnl), max.max(p.pnl))
            });

            let x0 = 40.0;
            let y0 = 20.0;
            let w = rect.width() - x0 - 10.0;
            let h = rect.height() - y0 - 30.0;

            // Draw axes
            ctx.stroke(Line::new(Point::new(x0, y0), Point::new(x0, y0 + h)), &PietColor::grey(0.7), 1.0);
            ctx.stroke(Line::new(Point::new(x0, y0 + h), Point::new(x0 + w, y0 + h)), &PietColor::grey(0.7), 1.0);

            // Draw PnL line
            if data.len() > 1 && (max_x - min_x).abs() > 1e-8 && (max_y - min_y).abs() > 1e-8 {
                let mut last = None;
                for p in data.iter() {
                    let px = x0 + ((p.time - min_x) / (max_x - min_x) * w as f64) as f64;
                    let py = y0 + h - ((p.pnl - min_y) / (max_y - min_y) * h as f64);
                    let pt = Point::new(px, py);
                    if let Some(prev) = last {
                        ctx.stroke(Line::new(prev, pt), &PietColor::rgb8(0x2a, 0x7b, 0xde), 2.0);
                    }
                    last = Some(pt);
                }
            }

            // Draw min/max/zero labels
            let text = ctx.text();
            let min_label = format!("{:.2}", min_y);
            let max_label = format!("{:.2}", max_y);
            let zero_y = if min_y < 0.0 && max_y > 0.0 {
                Some(y0 + h - ((0.0 - min_y) / (max_y - min_y) * h as f64))
            } else {
                None
            };
            let min_layout = text.new_text_layout(min_label).font(druid::FontFamily::SYSTEM_UI, 10.0).build().unwrap();
            let max_layout = text.new_text_layout(max_label).font(druid::FontFamily::SYSTEM_UI, 10.0).build().unwrap();
            ctx.draw_text(&min_layout, Point::new(2.0, y0 + h - 7.0));
            ctx.draw_text(&max_layout, Point::new(2.0, y0 - 7.0));
            if let Some(zero_y) = zero_y {
                ctx.stroke(Line::new(Point::new(x0, zero_y), Point::new(x0 + w, zero_y)), &PietColor::grey(0.8), 1.0);
                let zero_layout = text.new_text_layout("0.00").font(druid::FontFamily::SYSTEM_UI, 10.0).build().unwrap();
                ctx.draw_text(&zero_layout, Point::new(2.0, zero_y - 7.0));
            }
        }
    }

    fn pnl_chart() -> impl Widget<Vector<PnLPoint>> {
        PnLChartWidget
    }
    Label::dynamic(|data: &Vector<PnLPoint>, _| {
        if data.is_empty() {
            "PnL Chart (no data)".to_string()
        } else {
            format!("PnL Chart: {} points", data.len())
        }
    })
}

// Market Feed List UI

fn market_feed_list() -> impl Widget<Vector<MarketFeedItem>> {
    List::new(|| {
        Flex::row()
            .with_child(Label::new(|item: &MarketFeedItem, _| item.symbol.clone()).fix_width(80.0))
            .with_child(Label::new(|item: &MarketFeedItem, _| format!("{:.2}", item.price)).fix_width(80.0))
            .with_child(Label::new(|item: &MarketFeedItem, _| format!("{:+.2}%", item.change)).fix_width(80.0))
    })
}

// Positions Tab

fn positions_ui() -> impl Widget<AppState> {
    Flex::column()
        .with_child(
            Flex::row()
                .with_child(TextBox::new().with_placeholder("Filter symbol...").lens(AppState::positions.then(PositionsState::filter)))
                .with_spacer(10.0)
                .with_child(
                    Either::new(
                        |data: &AppState, _| {
                            data.user.as_ref().map(|u| u.role.can_trade()).unwrap_or(false)
                        },
                        Button::new("Buy").on_click(|_ctx, data: &mut AppState, _| {
                            // Show buy dialog or logic
                            data.error = Some("Buy dialog not implemented".into());
                        }),
                        SizedBox::empty(),
                    )
                )
                .with_spacer(10.0)
                .with_child(
                    Either::new(
                        |data: &AppState, _| {
                            data.user.as_ref().map(|u| u.role.can_trade()).unwrap_or(false)
                        },
                        Button::new("Sell").on_click(|_ctx, data: &mut AppState, _| {
                            // Show sell dialog or logic
                            data.error = Some("Sell dialog not implemented".into());
                        }),
                        SizedBox::empty(),
                    )
                )
        )
        .with_spacer(10.0)
        .with_child(
            Scroll::new(positions_table())
                .vertical()
                .lens(AppState::positions.then(PositionsState::positions))
        )
        .padding(10.0)
}

// Positions Table UI

fn positions_table() -> impl Widget<Vector<Position>> {
    List::new(|| {
        Flex::row()
            .with_child(Label::new(|p: &Position, _| p.symbol.clone()).fix_width(80.0))
            .with_child(Label::new(|p: &Position, _| format!("{}", p.quantity)).fix_width(60.0))
            .with_child(Label::new(|p: &Position, _| format!("{:.2}", p.average_price)).fix_width(80.0))
            .with_child(Label::new(|p: &Position, _| format!("{:.2}", p.realized_pnl)).fix_width(80.0))
            .with_child(Label::new(|p: &Position, _| format!("{:.2}", p.unrealized_pnl)).fix_width(80.0))
            .with_child(Label::new(|p: &Position, _| format!("{:.2}", p.last_traded_price)).fix_width(80.0))
    })
}

// Trades Tab   

fn trades_ui() -> impl Widget<AppState> {
    Flex::column()
        .with_child(
            Flex::row()
                .with_child(TextBox::new().with_placeholder("Filter...").lens(AppState::trades.then(TradesState::filter)))
        )
        .with_spacer(10.0)
        .with_child(
            Scroll::new(trades_table())
                .vertical()
                .lens(AppState::trades.then(TradesState::trades))
        )
        .padding(10.0)
}

// Trades Table UI

fn trades_table() -> impl Widget<Vector<Trade>> {
    List::new(|| {
        Flex::row()
            .with_child(Label::new(|t: &Trade, _| t.symbol.clone()).fix_width(80.0))
            .with_child(Label::new(|t: &Trade, _| format!("{:.2}", t.price)).fix_width(80.0))
            .with_child(Label::new(|t: &Trade, _| format!("{}", t.quantity)).fix_width(60.0))
            .with_child(Label::new(|t: &Trade, _| match t.action { TradeAction::Buy => "Buy", TradeAction::Sell => "Sell" }).fix_width(60.0))
            .with_child(Label::new(|t: &Trade, _| format!("{:.2}", t.pnl)).fix_width(80.0))
            .with_child(Label::new(|t: &Trade, _| t.date.clone()).fix_width(120.0))
    })
}

// Settings Tab

fn settings_ui() -> impl Widget<AppState> {
    Flex::column()
        .with_child(Label::new("Settings").with_text_size(24.0))
        .with_spacer(10.0)
        .with_child(
            Flex::row()
                .with_child(Label::new("API Key:").fix_width(80.0))
                .with_child(TextBox::new().lens(AppState::settings.then(SettingsState::api_key)))
        )
        .with_spacer(10.0)
        .with_child(
            Flex::row()
                .with_child(Label::new("Log Level:").fix_width(80.0))
                .with_child(RadioGroup::row(vec![
                    ("Debug", "debug"),
                    ("Info", "info"),
                    ("Warn", "warn"),
                    ("Error", "error"),
                ]).lens(AppState::settings.then(SettingsState::log_level)))
        )
        .with_spacer(10.0)
        .with_child(
            Flex::row()
                .with_child(Checkbox::new("Enable Logging").lens(AppState::settings.then(SettingsState::log_enabled)))
        )
        .padding(10.0)
}

//Backtesting Tab

fn backtesting_ui() -> impl Widget<AppState> {
    Flex::column()
        .with_child(Label::new("Backtesting Engine").with_text_size(24.0))
        .with_spacer(10.0)
        .with_child(
            Flex::row()
                .with_child(Label::new("Algorithm:").fix_width(80.0))
                .with_child(TextBox::new().with_placeholder("Algorithm name...").lens(AppState::backtesting.then(BacktestingState::selected_algorithm)))
                .with_spacer(20.0)
                .with_child(Label::new("Account:").fix_width(60.0))
                .with_child(
                    ComboBox::new(|| {
                        Box::new(|data: &AppState, _env| {
                            data.accounts.iter().map(|acc| acc.name.clone()).collect::<Vec<_>>()
                        })
                    })
                    .lens(AppState::backtesting.then(BacktestingState::selected_account))
                )
        )
        .with_spacer(10.0)
        .with_child(
            Flex::row()
                .with_child(Label::new("Start Date:").fix_width(80.0))
                .with_child(TextBox::new().with_placeholder("YYYY-MM-DD").lens(AppState::backtesting.then(BacktestingState::start_date)))
                .with_spacer(20.0)
                .with_child(Label::new("End Date:").fix_width(60.0))
                .with_child(TextBox::new().with_placeholder("YYYY-MM-DD").lens(AppState::backtesting.then(BacktestingState::end_date)))
        )
        .with_spacer(10.0)
        .with_child(
            Flex::row()
                .with_child(Button::new("Run Backtest").on_click(|_ctx, data: &mut AppState, _| {
                // Real backtesting architecture: spawn async task, update UI via progress callback

                use druid::{Selector, Target};

                // Define selectors for UI updates
                const BACKTEST_PROGRESS: Selector<f64> = Selector::new("backtest.progress");
                const BACKTEST_RESULT: Selector<Result<BacktestResult, String>> = Selector::new("backtest.result");

                // Mark as running and reset state
                data.backtesting.is_running = true;
                data.backtesting.progress = 0.0;
                data.backtesting.result = None;
                data.backtesting.error = None;

                // Clone necessary data for async move
                let backtest_service = data.backtest_service.clone();
                let algorithm = data.backtesting.selected_algorithm.clone();
                let account = data.backtesting.selected_account.clone();
                let start_date = data.backtesting.start_date.clone();
                let end_date = data.backtesting.end_date.clone();
                let accounts = data.accounts.clone();
                let event_sink = ctx.get_external_handle();

                // Spawn the backtest in a background thread or async task
                std::thread::spawn(move || {
                    // Progress callback closure
                    let mut progress_callback = |progress: f64| {
                        // Send progress update to UI
                        let _ = event_sink.submit_command(BACKTEST_PROGRESS, progress, Target::Auto);
                    };

                    // Run the backtest
                    let result = backtest_service.run_backtest(
                        &algorithm,
                        &account,
                        &start_date,
                        &end_date,
                        &accounts,
                        Some(&mut progress_callback),
                    );

                    // Send result to UI
                    let _ = event_sink.submit_command(BACKTEST_RESULT, result, Target::Auto);
                });

                // Async Command Handling for Backtest UI Updates
                pub const BACKTEST_PROGRESS: Selector<f64> = Selector::new("backtest.progress");
                pub const BACKTEST_RESULT: Selector<Result<BacktestResult, String>> = Selector::new("backtest.result");

                pub struct Delegate;

                impl AppDelegate<AppState> for Delegate {
                    fn command(
                        &mut self,
                        ctx: &mut DelegateCtx,
                        target: Target,
                        cmd: &Command,
                        data: &mut AppState,
                        _env: &Env,
                    ) -> Handled {
                        if let Some(progress) = cmd.get(BACKTEST_PROGRESS) {
                            data.backtesting.progress = *progress;
                            ctx.request_update();
                            return Handled::Yes;
                        }
                        if let Some(result) = cmd.get(BACKTEST_RESULT) {
                            data.backtesting.is_running = false;
                            data.backtesting.progress = 1.0;
                            match result {
                                Ok(res) => {
                                    data.backtesting.result = Some(res.clone());
                                    data.backtesting.error = None;
                                }
                                Err(e) => {
                                    data.backtesting.result = None;
                                    data.backtesting.error = Some(e.clone());
                                }
                            }
                            ctx.request_update();
                            return Handled::Yes;
                        }
                        Handled::No
                    }
                }

                    // Concrete implementation of the BacktestService
                    pub struct BacktestEngine;

                    impl BacktestService for BacktestEngine {
                        fn run_backtest(
                            &self,
                            algorithm: &str,
                            account: &str,
                            start_date: &str,
                            end_date: &str,
                            accounts: &[Account],
                            mut progress_callback: Option<&mut dyn FnMut(f64)>,
                        ) -> Result<BacktestResult, String> {
                            // 1. Find the account
                            let account = accounts.iter().find(|a| a.name == account)
                                .ok_or_else(|| format!("Account '{}' not found", account))?;

                            // 2. Load historical data for the account and date range
                            let historical_data = load_historical_data(account, start_date, end_date)
                                .map_err(|e| format!("Failed to load historical data: {}", e))?;

                            // 3. Initialize the algorithm
                            let mut algorithm = Algorithm::from_name(algorithm)
                                .ok_or_else(|| format!("Algorithm '{}' not found", algorithm))?;

                            // 4. Initialize backtest state
                            let mut state = BacktestState::new(account, &historical_data);

                            // 5. Run the backtest loop
                            let total = historical_data.len();
                            for (i, bar) in historical_data.iter().enumerate() {
                                algorithm.on_bar(bar, &mut state);
                                state.bar_index = i;
                                // Progress reporting for UI feedback
                                if let Some(cb) = progress_callback.as_mut() {
                                    cb((i + 1) as f64 / total.max(1) as f64);
                                }
                            }

                            // 6. Finalize and return result
                            let result = state.finalize();
                            Ok(result)
                        }
                    }

                    // Advanced dependency injection (DI) for the backtest service using trait objects and a DI container pattern.

                    use std::sync::Arc;

                    // Define a trait for the service (already shown above, but for clarity)
                    pub trait BacktestService: Send + Sync {
                        fn run_backtest(
                            &self,
                            algorithm: &str,
                            account: &str,
                            start_date: &str,
                            end_date: &str,
                            accounts: &[Account],
                            progress_callback: Option<&mut dyn FnMut(f64)>,
                        ) -> Result<BacktestResult, String>;
                    }

                    // DI Container struct to hold all services
                    pub struct ServiceContainer {
                        pub backtest_service: Arc<dyn BacktestService>,
                        // Add other services here as needed
                    }

                    impl ServiceContainer {
                        pub fn new() -> Self {
                            ServiceContainer {
                                backtest_service: Arc::new(BacktestEngine),
                                // Initialize other services here
                            }
                        }

                        // Optionally, provide accessors for services
                        pub fn backtest_service(&self) -> Arc<dyn BacktestService> {
                            Arc::clone(&self.backtest_service)
                        }
                    }


                    // When initializing your app:
                    let services = Arc::new(ServiceContainer::new());
                    let app_context = AppContext {
                        services: Arc::clone(&services),
                        // ... initialize other fields
                    };

                    // Usage:
                    // app_context.services.backtest_service().run_backtest(...);
                    fn get_backtest_service() -> &'static BacktestEngine {
                        static INSTANCE: BacktestEngine = BacktestEngine;
                        &INSTANCE
                    }

                    // UI event handler for running the backtest
                    {
                        let algorithm = data.backtesting.selected_algorithm.clone();
                        let account = data.backtesting.selected_account.clone();
                        let start_date = data.backtesting.start_date.clone();
                        let end_date = data.backtesting.end_date.clone();
                        let accounts = data.accounts.clone();

                        data.backtesting.is_running = true;
                        data.backtesting.progress = 0.0;
                        data.backtesting.result = None;
                        data.backtesting.error = None;
                        
                        // This assumes `app_state_arc` is initialized at the top of the file and accessible here.

                        let service = get_backtest_service();

                        // Clone the necessary data for the thread
                        let algorithm = algorithm.clone();
                        let account = account.clone();
                        let start_date = start_date.clone();
                        let end_date = end_date.clone();
                        let accounts = accounts.clone();

                        // Set running state before starting the thread
                        {
                            let mut data = app_state_arc.lock().unwrap();
                            data.backtesting.is_running = true;
                            data.backtesting.progress = 0.0;
                            data.backtesting.result = None;
                            data.backtesting.error = None;
                        }

                        let app_state_clone = Arc::clone(&app_state_arc);

                        thread::spawn(move || {
                            let mut progress = |p: f64| {
                                if let Ok(mut data) = app_state_clone.lock() {
                                    data.backtesting.progress = p;
                                }
                            };

                            let result = service.run_backtest(
                                &algorithm,
                                &account,
                                &start_date,
                                &end_date,
                                &accounts,
                                Some(&mut progress),
                            );

                            if let Ok(mut data) = app_state_clone.lock() {
                                match result {
                                    Ok(result) => {
                                        data.backtesting.result = Some(result);
                                        data.backtesting.is_running = false;
                                        data.backtesting.progress = 1.0;
                                        data.backtesting.error = None;
                                    }
                                    Err(e) => {
                                        data.backtesting.result = None;
                                        data.backtesting.is_running = false;
                                        data.backtesting.progress = 0.0;
                                        data.backtesting.error = Some(format!("Backtest failed: {}", e));
                                    }
                                }
                            }
                        });

                    }
                }))
                .with_spacer(10.0)
                .with_child(Button::new("Reset").on_click(|_ctx, data: &mut AppState, _| {
                    data.backtesting.result = None;
                    data.backtesting.progress = 0.0;
                    data.backtesting.is_running = false;
                    data.backtesting.error = None;
                }))
        )
        .with_spacer(10.0)
        .with_child(
            Either::new(
                |data: &AppState, _| data.backtesting.is_running,
                Label::dynamic(|data: &AppState, _| format!("Backtest running... {:.0}%", data.backtesting.progress * 100.0)),
                SizedBox::empty(),
            )
        )
        .with_spacer(10.0)
        .with_child(
            Either::new(
                |data: &AppState, _| data.backtesting.result.is_some(),
                Label::dynamic(|data: &AppState, _| {
                    if let Some(ref result) = data.backtesting.result {
                        format!("Backtest Result: {}", result.summary)
                    } else {
                        "".into()
                    }
                }),
                SizedBox::empty(),
            )
        )
        .padding(10.0)
}

// User Management Tab (Admin only)

fn user_management_ui() -> impl Widget<AppState> {
    Label::new("User Management (Admin Only) - Not Implemented")
        .with_text_size(18.0)
        .padding(10.0)
}

// Error Feedback (Status Bar)

fn status_bar() -> impl Widget<AppState> {
    Either::new(
        |data: &AppState, _| data.error.is_some() || data.ide.error.is_some() || data.backtesting.error.is_some(),
        Label::dynamic(|data: &AppState, _| {
            data.error.clone()
                .or_else(|| data.ide.error.clone())
                .or_else(|| data.backtesting.error.clone())
                .unwrap_or_default()
        })
            .with_text_color(Color::rgb8(200, 0, 0))
            .padding(5.0),
        SizedBox::empty(),
    )
}
