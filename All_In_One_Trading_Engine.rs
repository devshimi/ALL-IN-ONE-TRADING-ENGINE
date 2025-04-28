// --- Imports and Dependencies ---
// Druid GUI and widgets
use druid::{
    AppLauncher, Data, Lens, Widget, WidgetExt, WindowDesc, Color, LocalizedString,
    widget::{Flex, Label, Button, TextBox, List, Tabs, TabsPolicy, ViewSwitcher, Checkbox, RadioGroup, SizedBox, Scroll, Either, Container, Split, Controller, Painter, ComboBox, ProgressBar, Tooltip},
    AppDelegate, DelegateCtx, Handled, Selector, Command, Target,
};

// Concurrency and Async Primitives 
use druid::im::Vector;// Immutable vector for app state
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
use simplelog::{ // Flexible logging to terminal and file
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
use std::fmt; // Formatting traits (Display, Debug, etc.)
use std::io; // IO traits and types
use std::result::Result as StdResult; // Alias for std::result::Result

// Redundant Imports 
use simplelog::{ 
    ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger,
    ThreadLogMode, LevelPadding, Record, Config,
};
use chrono::Local; // 
use std::fs::File; // File operations 
use std::io::Write; // For writing logs/files
use tokio::sync::{mpsc, broadcast, oneshot, RwLock}; // Async channels and RwLock for state
use tokio::task; // Async task spawning
use tokio::time::sleep; // Async sleep/delay

// IBKR Client Integration 
use ibkr_client::{
    TwsClient, TwsClientConfig, TwsError,
    contract::Contract,
    order::Order as IbkrOrder,
    event::Event as IbkrEvent,
};

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

// Global AppState Arc for background threads 
use std::thread;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
static APP_STATE_ARC: Lazy<Arc<Mutex<AppState>>> = Lazy::new(|| Arc::new(Mutex::new(AppState::new())));

/// Application context: Dependency injection container for services and shared state
pub struct AppContext {
    pub services: Arc<ServiceContainer>,        // All DI services 
    pub user_session: Option<UserSession>,      // Currently logged-in user session 
    pub config: AppConfig,                      // Application-wide configuration
    pub logger: Arc<dyn Logger>,                // Centralized logger 
    pub shared_state: Arc<Mutex<SharedState>>,  // Global mutable state (UI, background tasks, etc.)
    pub cache: Arc<Mutex<Cache>>,               // Optional: cache for fast lookups
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

/// Unified, extensible error type for the entire application (for robust error handling)
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
    /// Create a new connection manager with shared state
    pub fn new(state: Arc<RwLock<IbkrState>>) -> Self {
        Self {
            state,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the connection manager in a background async task
    pub fn start(self: Arc<Self>) {
        let manager = self.clone();
        task::spawn(async move {
            manager.run().await;
        });
    }

    /// Main connection loop: handles connect, reconnect, event dispatch, error handling
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

    /// Request shutdown of the connection manager
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

// Encryption 

use ring::aead; // Symmetric encryption
use ring::rand::{SystemRandom, SecureRandom}; // Random for nonce
use base64::{encode as b64encode, decode as b64decode}; // Base64 for key/ciphertext

/// Simple symmetric encryptor for secure storage (key from env)
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

#[derive(Clone, Data, Lens)]
pub struct DomData {
    pub symbol: String, // Symbol
    pub bids: Vector<DomLevel>, // Bid levels
    pub asks: Vector<DomLevel>, // Ask levels
}

#[derive(Clone, Data, Lens)]
pub struct DomLevel {
    pub price: f64, // Price level
    pub size: i32, // Size at level
}

#[derive(Clone, Data, Lens)]
pub struct TAndSData {
    pub symbol: String, // Symbol
    pub trades: Vector<TimeAndSales>, // Time & Sales records
}

#[derive(Clone, Data, Lens)]
pub struct TimeAndSales {
    pub price: f64, // Trade price
    pub size: i32, // Trade size
    pub time: String, // Time of trade
    pub side: String, // Buy/Sell
}

#[derive(Clone, Data, Lens)]
pub struct SecFiling {
    pub symbol: String, // Symbol
    pub filing_type: String, // Filing type (e.g., 10-K)
    pub date: String, // Filing date
    pub url: String, // Filing URL
    pub description: String, // Description
}

// Position, Trade, MarketFeed, PnLPoint 
#[derive(Clone, Data, Lens)]
pub struct Position {
    pub symbol: String, // Symbol
    pub quantity: i32, // Quantity held
    pub average_price: f64, // Average entry price
    pub realized_pnl: f64, // Realized PnL
    pub unrealized_pnl: f64, // Unrealized PnL
    pub last_traded_price: f64, // Last traded price
}

#[derive(Clone, Data, Lens)]
pub struct Trade {
    pub symbol: String, // Symbol
    pub price: f64, // Trade price
    pub quantity: i32, // Quantity
    pub action: TradeAction, // Buy/Sell
    pub pnl: f64, // PnL for this trade
    pub date: String, // Trade date
}

#[derive(Clone, Data, PartialEq, Eq, Lens)]
pub enum TradeAction {
    Buy,
    Sell,
}

#[derive(Clone, Data, Lens)]
pub struct MarketFeedItem {
    pub symbol: String, // Symbol
    pub price: f64, // Last price
    pub change: f64, // % change
}

#[derive(Clone, Data, Lens)]
pub struct PnLPoint {
    pub time: f64, // Time (for charting)
    pub pnl: f64, // PnL value
    pub value: f64, // (Optional, for compatibility)
}

// RBAC (Role-Based Access Control)

impl UserRole {
    /// Manage users (admin only)
    pub fn can_manage_users(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
    /// Trade (admin or trader)
    pub fn can_trade(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Trader)
    }
    /// View positions (all except unknown)
    pub fn can_view_positions(&self) -> bool {
        !matches!(self, UserRole::Unknown)
    }
    /// View logs (admin or analyst)
    pub fn can_view_logs(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Analyst)
    }
    /// Edit algorithms (admin, trader, analyst)
    pub fn can_edit_algorithms(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Trader | UserRole::Analyst)
    }
    /// Manage bots (admin or trader)
    pub fn can_manage_bots(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Trader)
    }
    /// View DOM (admin or trader)
    pub fn can_view_dom(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Trader)
    }
}

// Main Entry Point GUI

fn main() {
    // Setup logger
    let _ = setup_logger(LevelFilter::Info);

    // Main window and initial state
    let main_window = WindowDesc::new(ui_root)
        .title(LocalizedString::new("Trading Algorithm IDE"))
        .window_size((1500.0, 950.0));
    let initial_state = AppState {
        user: None,
        login: LoginState {
            username: "".into(),
            password: "".into(),
            error: None,
        },
        dashboard: DashboardState {
            pnl_history: Vector::new(),
            market_feed: Vector::new(),
        },
        positions: PositionsState {
            positions: Vector::new(),
            filter: "".into(),
        },
        trades: TradesState {
            trades: Vector::new(),
            filter: "".into(),
        },
        settings: SettingsState {
            api_key: "".into(),
            log_level: "info".into(),
            log_enabled: true,
        },
        ide: IdeState::default(),
        accounts: Vector::from(vec![TradingAccountState::default()]),
        selected_account: 0,
        backtesting: BacktestingState::default(),
        error: None,
        alerts: Vector::new(),
        bots: Vector::new(),
        orders: Vector::new(),
        option_chain: None,
        dom: None,
        t_and_s: None,
        sec_filings: Vector::new(),
        notification: None,
        ibkr: IbkrState::default(),
    };
    AppLauncher::with_window(main_window)
        .launch(initial_state)
        .expect("Failed to launch app");
}

// UI Root (Account selector, login/main switch, status bar, notification bar, IBKR connect bar)

fn ui_root() -> impl Widget<AppState> {
    Flex::column()
        .with_child(account_selector_ui()) // Account selection at top
        .with_child(ibkr_connect_bar())    // IBKR connect bar
        .with_flex_child(
            ViewSwitcher::new(
                |data: &AppState, _| data.user.is_some(),
                |is_logged_in, _| {
                    if *is_logged_in {
                        main_app_ui().boxed() // Main app tabs
                    } else {
                        login_ui().boxed() // Login screen
                    }
                },
            ),
            1.0,
        )
        .with_child(status_bar()) // Error/status bar at bottom
        .with_child(notification_bar()) // New: notification bar for runtime feedback
}

// IBKR Connect Bar UI

fn ibkr_connect_bar() -> impl Widget<AppState> {
    Flex::row()
        .with_child(Label::new("IBKR Gateway:").with_text_size(14.0))
        .with_spacer(4.0)
        .with_child(TextBox::new().with_placeholder("Host").lens(AppState::ibkr.then(IbkrState::host)).fix_width(120.0))
        .with_spacer(4.0)
        .with_child(TextBox::new().with_placeholder("Port").lens(AppState::ibkr.then(IbkrState::port)).fix_width(60.0))
        .with_spacer(4.0)
        .with_child(TextBox::new().with_placeholder("Client ID").lens(AppState::ibkr.then(IbkrState::client_id)).fix_width(60.0))
        .with_spacer(8.0)
        .with_child(
            Either::new(
                |data: &AppState, _| data.ibkr.is_connected,
                Button::new("Disconnect").on_click(|_ctx, data: &mut AppState, _| {
                    // Fully implemented disconnect logic
                    if let Some(mut client) = data.ibkr.client.take() {
                        // Attempt to disconnect gracefully
                        if let Err(e) = client.disconnect() {
                            data.ibkr.error = Some(format!("Error disconnecting: {:?}", e));
                        } else {
                            data.ibkr.error = None;
                        }
                    } else {
                        data.ibkr.error = None;
                    }
                    data.ibkr.is_connected = false;
                    data.ibkr.is_logged_in = false;
                    data.ibkr.last_event = Some("Disconnected from IBKR".into());
                }),
                Button::new("Connect").on_click(|ctx, data: &mut AppState, _| {
                    // Connect to IBKR TWS or Gateway
                    let host = data.ibkr.host.clone();
                    let port = data.ibkr.port;
                    let client_id = data.ibkr.client_id;
                    let ibkr_state = Arc::new(Mutex::new(data.ibkr.clone()));
                    let (tx, rx): (Sender<IbkrEvent>, Receiver<IbkrEvent>) = mpsc::channel();

                    // Spawn a thread to connect and listen for events
                    thread::spawn({
                        let ibkr_state = ibkr_state.clone();
                        move || {
                            let config = TwsClientConfig::default()
                                .host(host)
                                .port(port)
                                .client_id(client_id);
                            match TwsClient::connect(config) {
                                Ok(mut client) => {
                                    {
                                        let mut state = ibkr_state.lock().unwrap();
                                        state.is_connected = true;
                                        state.error = None;
                                        state.last_event = Some("Connected to IBKR".into());
                                    }
                                    //Subscribe to required IBKR streams, process, events, updates and market data
                                    if let Err(e) = client.req_account_updates(true, &data.ibkr.account_code) {
                                        let mut state = ibkr_state.lock().unwrap();
                                        state.error = Some(format!("Failed to subscribe to account updates: {:?}", e));
                                    }

                                    // You can add more subscriptions here as needed, e.g.:
                                    for event in client.events() {
                                        // Forward event to main thread for UI/state update
                                        if tx.send(event.clone()).is_err() {
                                            // Channel closed, exit loop
                                            break;
                                        }

                                        // Optionally, process important events here
                                        match &event {
                                            IbkrEvent::Error { code, message } => {
                                                let mut state = ibkr_state.lock().unwrap();
                                                state.error = Some(format!("IBKR Error {}: {}", code, message));
                                            }
                                            IbkrEvent::ConnectionClosed => {
                                                let mut state = ibkr_state.lock().unwrap();
                                                state.is_connected = false;
                                                state.is_logged_in = false;
                                                state.last_event = Some("Connection closed by IBKR".into());
                                                break;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                Err(e) => {
                                    let mut state = ibkr_state.lock().unwrap();
                                    state.is_connected = false;
                                    state.error = Some(format!("IBKR connect error: {:?}", e));
                                }
                            }
                        }
                    });

                    // Create a Tokio runtime for async event processing
                    let rt = Runtime::new().expect("Failed to create Tokio runtime");
                    let ibkr_state = ibkr_state.clone();

                    // Replace std::sync::mpsc::Receiver with tokio::sync::mpsc::Receiver for async
                    let (async_tx, mut async_rx) = mpsc::unbounded_channel();

                    // Forward all events from the original rx to the async channel
                    thread::spawn({
                        let async_tx = async_tx.clone();
                        move || {
                            for event in rx {
                                if async_tx.send(event).is_err() {
                                    break;
                                }
                            }
                        }
                    });

                    // Spawn an async task to process events in a granular, event-driven way
                    rt.spawn({
                        let ibkr_state = ibkr_state.clone();
                        async move {
                            while let Some(event) = async_rx.recv().await {
                                let mut state = ibkr_state.lock().unwrap();
                                // Advanced event parsing and state update
                                match &event {
                                    IbkrEvent::AccountUpdate { account, key, value, .. } => {
                                        // Update account info in state
                                        state.last_event = Some(format!("AccountUpdate: {} {}={}", account, key, value));
                                        // Optionally update positions, balances, etc.
                                    }
                                    IbkrEvent::OrderStatus { order_id, status, filled, remaining, avg_fill_price, .. } => {
                                        // Update order status in state
                                        state.last_event = Some(format!(
                                            "OrderStatus: id={} status={} filled={} remaining={} avg_price={}",
                                            order_id, status, filled, remaining, avg_fill_price
                                        ));
                                        // Optionally update order book, positions, etc.
                                    }
                                    IbkrEvent::Execution { exec_id, symbol, side, shares, price, .. } => {
                                        // Update executions in state
                                        state.last_event = Some(format!(
                                            "Execution: id={} {} {}@{} {}",
                                            exec_id, symbol, shares, price, side
                                        ));
                                        // Optionally update trade history, realized PnL, etc.
                                    }
                                    IbkrEvent::MarketData { symbol, bid, ask, last, volume, .. } => {
                                        // Update market data in state
                                        state.last_event = Some(format!(
                                            "MarketData: {} bid={} ask={} last={} vol={}",
                                            symbol, bid, ask, last, volume
                                        ));
                                        // Optionally update DOM, T&S, etc.
                                    }
                                    IbkrEvent::Error { code, message } => {
                                        state.error = Some(format!("IBKR Error {}: {}", code, message));
                                        state.last_event = Some(format!("Error: {} - {}", code, message));
                                    }
                                    IbkrEvent::ConnectionClosed => {
                                        state.is_connected = false;
                                        state.is_logged_in = false;
                                        state.last_event = Some("Connection closed by IBKR".into());
                                    }
                                    _ => {
                                        // Generic event fallback
                                        state.last_event = Some(format!("{:?}", event));
                                    }
                                }
                                // Set is_logged_in only for authenticated/valid events
                                if matches!(event, IbkrEvent::AccountUpdate { .. } | IbkrEvent::OrderStatus { .. } | IbkrEvent::Execution { .. }) {
                                    state.is_logged_in = true;
                                }
                            }
                        }
                    });

                    // Set state in AppState (for UI update)
                    data.ibkr.is_connected = true;
                    data.ibkr.error = None;
                    data.ibkr.last_event = Some("Connecting to IBKR...".into());
                    ctx.request_update();
                }),
            )
        )
        .with_spacer(8.0)
        .with_child(
            Either::new(
                |data: &AppState, _| data.ibkr.is_connected,
                Label::new(|data: &AppState, _| {
                    if data.ibkr.is_logged_in {
                        "IBKR: Connected & Authenticated".to_string()
                    } else {
                        "IBKR: Connected (not authenticated)".to_string()
                    }
                }).with_text_color(Color::rgb8(0, 180, 0)),
                Label::new("IBKR: Disconnected").with_text_color(Color::rgb8(180, 0, 0)),
            )
        )
        .with_spacer(8.0)
        .with_child(
            Either::new(
                |data: &AppState, _| data.ibkr.error.is_some(),
                Label::dynamic(|data: &AppState, _| data.ibkr.error.clone().unwrap_or_default())
                    .with_text_color(Color::rgb8(200, 0, 0)),
                SizedBox::empty(),
            )
        )
        .with_spacer(8.0)
        .with_child(
            Either::new(
                |data: &AppState, _| data.ibkr.last_event.is_some(),
                Label::dynamic(|data: &AppState, _| data.ibkr.last_event.clone().unwrap_or_default())
                    .with_text_color(Color::rgb8(0, 0, 180)),
                SizedBox::empty(),
            )
        )
        .padding((10.0, 4.0, 10.0, 4.0))
}

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

// Login Screen UI

fn login_ui() -> impl Widget<AppState> {
    let username = TextBox::new()
        .with_placeholder("Username")
        .lens(AppState::login.then(LoginState::username))
        .with_tooltip("Enter your username");
    let password = TextBox::new()
        .with_placeholder("Password")
        .lens(AppState::login.then(LoginState::password))
        .with_tooltip("Enter your password");
    let error_label = Label::dynamic(|data: &AppState, _| {
        data.login.error.clone().unwrap_or_default()
    })
    .with_text_color(Color::rgb8(200, 0, 0));
    let login_btn = Button::new("Login")
        .on_click(|ctx, data: &mut AppState, _env| {
            // Attempt to fetch user from the database and verify the password hash using bcrypt.

            // Async login: use a background thread for DB access, proper error handling, and bcrypt verification.
            // This uses a channel to communicate the result back to the UI thread.
            use diesel::prelude::*;
            use diesel::sqlite::SqliteConnection;
            use std::sync::mpsc;
            use std::thread;

            // Clone the login data for the thread
            let username = data.login.username.clone();
            let password = data.login.password.clone();
            let (tx, rx) = mpsc::channel();

            // Spawn a thread for DB and bcrypt work
            thread::spawn(move || {
                // Try to get a database connection
                let conn = match establish_connection() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(format!("Database error: {}", e)));
                        return;
                    }
                };

                // Define a struct to represent a user row from the database
                #[derive(Queryable)]
                struct DbUser {
                    pub id: i32,
                    pub username: String,
                    pub password_hash: String,
                    pub role: String,
                }

                // Try to find the user by username
                use self::users::dsl::*;
                let user_result = users
                    .filter(username.eq(&username))
                    .first::<DbUser>(&conn);

                match user_result {
                    Ok(db_user) => {
                        // Verify the password using bcrypt
                        match verify_password(&password, &db_user.password_hash) {
                            Ok(true) => {
                                let _ = tx.send(Ok(db_user));
                            }
                            Ok(false) => {
                                let _ = tx.send(Err("Invalid username or password".into()));
                            }
                            Err(e) => {
                                let _ = tx.send(Err(format!("Password verification error: {}", e)));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("User lookup error: {}", e)));
                    }
                }
            });

            // Wait for the result from the thread
            if let Ok(result) = rx.recv() {
                match result {
                    Ok(db_user) => {
                        // Map role string to UserRole
                        let user_role = match db_user.role.as_str() {
                            "admin" => UserRole::Admin,
                            "trader" => UserRole::Trader,
                            "readonly" => UserRole::ReadOnly,
                            "analyst" => UserRole::Analyst,
                            _ => UserRole::ReadOnly,
                        };
                        data.user = Some(User {
                            username: db_user.username,
                            role: user_role,
                            password_hash: Some(db_user.password_hash),
                        });
                        data.login.error = None;
                    }
                    Err(e) => {
                        data.login.error = Some(e);
                    }
                }
            }

            // Define a struct to represent a user row from the database
            #[derive(Queryable)]
            struct DbUser {
                pub id: i32,
                pub username: String,
                pub password_hash: String,
                pub role: String,
            }

            // Try to get a database connection
            let conn = match establish_connection() {
                Ok(c) => c,
                Err(e) => {
                    data.login.error = Some(format!("Database error: {}", e));
                    ctx.request_update();
                    return;
                }
            };

            // Try to find the user by username
            use self::users::dsl::*;
            let user_result = users
                .filter(username.eq(&data.login.username))
                .first::<DbUser>(&conn);

            match user_result {
                Ok(db_user) => {
                    // Verify the password using bcrypt
                    match verify_password(&data.login.password, &db_user.password_hash) {
                        Ok(true) => {
                            // Map role string to UserRole
                            let user_role = match db_user.role.as_str() {
                                "admin" => UserRole::Admin,
                                "trader" => UserRole::Trader,
                                "readonly" => UserRole::ReadOnly,
                                "analyst" => UserRole::Analyst,
                                _ => UserRole::ReadOnly,
                            };
                            data.user = Some(User {
                                username: db_user.username,
                                role: user_role,
                                password_hash: Some(db_user.password_hash),
                            });
                            data.login.error = None;
                        }
                        Ok(false) => {
                            data.login.error = Some("Invalid username or password".into());
                        }
                        Err(e) => {
                            data.login.error = Some(format!("Password verification error: {}", e));
                        }
                    }
                }
                Err(diesel::result::Error::NotFound) => {
                    data.login.error = Some("Invalid username or password".into());
                }
                Err(e) => {
                    data.login.error = Some(format!("Database error: {}", e));
                }
            }
            ctx.request_update();
        });
    Flex::column()
        .with_spacer(100.0)
        .with_child(Label::new("Login to Trading Algorithm IDE").with_text_size(32.0))
        .with_spacer(20.0)
        .with_child(username)
        .with_spacer(10.0)
        .with_child(password)
        .with_spacer(10.0)
        .with_child(login_btn)
        .with_spacer(10.0)
        .with_child(error_label)
        .center()
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
        1200.0, // width
        700.0,  // height
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
    use druid_code_editor::{CodeEditor, EditorState, Language};
    use druid::widget::{Flex, Label, Either, Controller, ControllerHost, WidgetExt, ComboBox};
    use druid::{Data, Lens};

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
