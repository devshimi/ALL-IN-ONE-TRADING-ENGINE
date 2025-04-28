// --- Imports and Dependencies ---
// Druid GUI and widgets
use druid::{
    AppLauncher, Data, Env, Lens, Widget, WidgetExt, WindowDesc, Color, LocalizedString,
    widget::{Flex, Label, Button, TextBox, List, Tabs, TabsPolicy, ViewSwitcher, Checkbox, RadioGroup, SizedBox, Scroll, Either, Container, Split, Controller, Painter, ComboBox, ProgressBar, Tooltip},
}; // GUI framework and widgets
use druid::im::Vector; // Immutable vector for app state
use std::sync::Arc; // Thread-safe reference counting
use chrono::{DateTime, Utc}; // Date/time handling

// Error Handling
use thiserror::Error; // Error derive macro
use std::io; // IO errors
use std::fmt; // Formatting
use std::result::Result as StdResult; // Result alias
use bcrypt::{hash, verify, DEFAULT_COST};// Password hashing and verification using bcrypt

/// Returns the hashed password as a String, or an error if hashing fails.
pub fn hash_password(plain: &str) -> Result<String, bcrypt::BcryptError> {
    hash(plain, DEFAULT_COST)
}

/// Verify a plaintext password against a bcrypt hash.
/// Returns true if the password matches, false otherwise.
pub fn verify_password(plain: &str, hashed: &str) -> Result<bool, bcrypt::BcryptError> {
    verify(plain, hashed)
}

// Error Handling  

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    ConfigError(String), // Configuration file or settings error
    #[error("Authentication error: {0}")]
    AuthError(String),   // Login/authentication error
    #[error("Database error: {0}")]
    DatabaseError(String), // Database connection/query error
    #[error("IO error: {0}")]
    IoError(#[from] io::Error), // File or IO error
    #[error("Encryption error: {0}")]
    EncryptionError(String), // Encryption/decryption error
    #[error("Market data error: {0}")]
    MarketDataError(String), // Market data feed error
    #[error("IBKR error: {0}")]
    IbkrError(String), // Interactive Brokers API error
    #[error("Alert error: {0}")]
    AlertError(String), // Alert system error
    #[error("Backtest error: {0}")]
    BacktestError(String), // Backtesting engine error
    #[error("Other error: {0}")]
    Other(String), // Any other error
}

pub type Result<T> = StdResult<T, AppError>; // Convenience alias for results

// Logger Setup 

use log::{info, warn, error, debug, LevelFilter}; // Logging macros
use simplelog::*; // Simplelog for logging
use std::fs::File; // File for log output

pub fn setup_logger(log_level: LevelFilter) -> Result<()> {
    // Setup logging to both terminal and file
    let log_file = File::create("trading_ide.log").map_err(|e| AppError::IoError(e))?;
    CombinedLogger::init(vec![
        TermLogger::new(log_level, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(log_level, Config::default(), log_file),
    ]).map_err(|e| AppError::Other(format!("Logger init failed: {:?}", e)))?;
    Ok(())
}

// Encryption 

use ring::aead; // Symmetric encryption
use ring::rand::{SystemRandom, SecureRandom}; // Random for nonce
use base64::{encode as b64encode, decode as b64decode}; // Base64 for key/ciphertext

pub struct Encryptor {
    key: Vec<u8>, // Symmetric key
}

impl Encryptor {
    // Create Encryptor from environment variable (base64 key)
    pub fn new_from_env() -> Result<Self> {
        let key = std::env::var("TRADING_IDE_KEY")
            .map_err(|_| AppError::EncryptionError("Missing encryption key in env".into()))?;
        let key_bytes = b64decode(&key).map_err(|_| AppError::EncryptionError("Invalid base64 key".into()))?;
        Ok(Self { key: key_bytes })
    }
    // Encrypt plaintext bytes, return base64 ciphertext
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
}

// Login form 
#[derive(Clone, Data, Lens)]
pub struct LoginState {
    pub username: String,
    pub password: String,
    pub error: Option<String>,
}

//  Dashboard state (PnL chart, market feed) 
#[derive(Clone, Data, Lens)]
pub struct DashboardState {
    pub pnl_history: Vector<PnLPoint>,
    pub market_feed: Vector<MarketFeedItem>,
}

// Positions tab state 
#[derive(Clone, Data, Lens)]
pub struct PositionsState {
    pub positions: Vector<Position>,
    pub filter: String,
}

// Trades tab 
#[derive(Clone, Data, Lens)]
pub struct TradesState {
    pub trades: Vector<Trade>,
    pub filter: String,
}

//  Settings tab  
#[derive(Clone, Data, Lens)]
pub struct SettingsState {
    pub api_key: String,
    pub log_level: String,
    pub log_enabled: bool,
}

// User and roles -
#[derive(Clone, Data, Lens)]
pub struct User {
    pub username: String,
    pub role: UserRole,
    pub password_hash: Option<String>, // For bcrypt
}

#[derive(Clone, Data, PartialEq, Eq)]
pub enum UserRole {
    Admin,      // Full access
    Trader,     // Can trade, edit algos
    ReadOnly,   // View only
    Analyst,    // Can view logs, edit algos
    Unknown,    // No permissions
}

// IDE/editor 
#[derive(Clone, Data, Lens)]
pub struct IdeState {
    pub files: Vector<IdeFile>, // Open files
    pub current_file: usize, // Index of current file
    pub console_output: String, // Output/print/debug
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
    pub name: String,
    pub content: String,
    pub path: Option<String>,
    pub is_dirty: bool, // Unsaved changes
    pub error_lines: Vector<usize>,
    pub language: String, // Syntax highlighting
    pub editor_state: druid_code_editor::EditorState,
}

// Variable watch (debugger)
#[derive(Clone, Data, Lens)]
pub struct VariableWatch {
    pub name: String,
    pub value: String,
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
    pub name: String,
    pub strategy: String,
    pub execution_rules: String,
    pub positions: Vector<Position>,
    pub trades: Vector<Trade>,
    pub balance: f64,
    pub is_active: bool,
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
    pub is_running: bool,
    pub progress: f64,
    pub selected_algorithm: Option<String>,
    pub selected_account: usize,
    pub start_date: String,
    pub end_date: String,
    pub result: Option<BacktestResult>,
    pub error: Option<String>,
    pub selected_strategy: String, 
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
    pub pnl_history: Vector<PnLPoint>,
    pub trades: Vector<Trade>,
    pub summary: String,
    pub sharpe_ratio: Option<f64>,
    pub max_drawdown: Option<f64>,
}

// Alert, Bot, Order, OptionChain, DOM, T&S, SEC Filing 
#[derive(Clone, Data, Lens)]
pub struct Alert {
    pub id: usize,
    pub symbol: String,
    pub condition: String,
    pub triggered: bool,
    pub last_triggered: Option<String>,
}

#[derive(Clone, Data, Lens)]
pub struct BotState {
    pub name: String,
    pub is_running: bool,
    pub strategy: String,
    pub log: String,
    pub config: String, 
}

#[derive(Clone, Data, Lens)]
pub struct Order {
    pub id: usize,
    pub symbol: String,
    pub price: f64,
    pub quantity: i32,
    pub status: String,
    pub order_type: String,
    pub date: String,
}

#[derive(Clone, Data, Lens)]
pub struct OptionChain {
    pub symbol: String,
    pub expirations: Vector<String>,
    pub strikes: Vector<f64>,
    pub calls: Vector<OptionContract>,
    pub puts: Vector<OptionContract>,
}

#[derive(Clone, Data, Lens)]
pub struct OptionContract {
    pub strike: f64,
    pub expiration: String,
    pub bid: f64,
    pub ask: f64,
    pub iv: f64,
    pub volume: i32,
    pub open_interest: i32,
    pub contract_type: String, // "call" or "put"
}

#[derive(Clone, Data, Lens)]
pub struct DomData {
    pub symbol: String,
    pub bids: Vector<DomLevel>,
    pub asks: Vector<DomLevel>,
}

#[derive(Clone, Data, Lens)]
pub struct DomLevel {
    pub price: f64,
    pub size: i32,
}

#[derive(Clone, Data, Lens)]
pub struct TAndSData {
    pub symbol: String,
    pub trades: Vector<TimeAndSales>,
}

#[derive(Clone, Data, Lens)]
pub struct TimeAndSales {
    pub price: f64,
    pub size: i32,
    pub time: String,
    pub side: String,
}

#[derive(Clone, Data, Lens)]
pub struct SecFiling {
    pub symbol: String,
    pub filing_type: String,
    pub date: String,
    pub url: String,
    pub description: String,
}

// Position, Trade, MarketFeed, PnLPoint 
#[derive(Clone, Data, Lens)]
pub struct Position {
    pub symbol: String,
    pub quantity: i32,
    pub average_price: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub last_traded_price: f64,
}

#[derive(Clone, Data, Lens)]
pub struct Trade {
    pub symbol: String,
    pub price: f64,
    pub quantity: i32,
    pub action: TradeAction,
    pub pnl: f64,
    pub date: String,
}

#[derive(Clone, Data, PartialEq, Eq, Lens)]
pub enum TradeAction {
    Buy,
    Sell,
}

#[derive(Clone, Data, Lens)]
pub struct MarketFeedItem {
    pub symbol: String,
    pub price: f64,
    pub change: f64,
}

#[derive(Clone, Data, Lens)]
pub struct PnLPoint {
    pub time: f64, // For charting, use f64 for time axis
    pub pnl: f64,
    pub value: f64, // For compatibility
}

// RBAC (Role-Based Access Control)

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
    };
    AppLauncher::with_window(main_window)
        .launch(initial_state)
        .expect("Failed to launch app");
}

// UI Root (Account selector, login/main switch, status bar, notification bar)

fn ui_root() -> impl Widget<AppState> {
    Flex::column()
        .with_child(account_selector_ui()) // Account selection at top
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
            // This is a simplified synchronous example; in a real app, use async and proper error handling.
            use diesel::prelude::*;
            use diesel::sqlite::SqliteConnection;

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
                    // Start backtest (dummy result for demo)
                    data.backtesting.is_running = true;
                    data.backtesting.progress = 0.0;
                    data.backtesting.result = None;
                    data.backtesting.error = None;
                    // Here you would trigger the actual backtest logic (simulate, update progress, etc.)
                    // For demo, just set a dummy result after a moment.
                    data.backtesting.result = Some(BacktestResult {
                        pnl_history: Vector::new(),
                        trades: Vector::new(),
                        summary: "Backtest completed (demo)".into(),
                        sharpe_ratio: None,
                        max_drawdown: None,
                    });
                    data.backtesting.is_running = false;
                    data.backtesting.progress = 1.0;
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
