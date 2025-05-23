# ALL IN ONE TRADING ENGINE

The All In One Trading Engine is designed to support a wide range of advanced trading strategies. With features like real-time charting, profit and loss (PnL) tracking, market data integration, and algorithmic trading, it provides a powerful platform for managing and optimizing trading strategies. The system integrates with Interactive Brokers (IBKR) for live market data and order management, making it adaptable for both individual traders and financial professionals seeking to enhance their trading operations.
## Getting Started

### Requirements 
- Rust 
- SQLite (for database integration)
- Required dependencies 

### Installation
Clone the repository:
```sh
git clone https://github.com/devshimi/ultimate-trading-app.git
cd ultimate-trading-app
```

Install the necessary dependencies (Rust package manager):
```sh
cargo build
```

### Running the Application
Start the application by running the following command:
```sh
cargo run
```

## Features Overview

### Core Features
- **Custom Exceptions**: Handles critical issues such as configuration errors, authentication failures, and database issues, ensuring robust error handling throughout the application.
- **Logger Setup**: Includes a rotating file logger and console stream handler to ensure detailed logging for all activities, making debugging and tracking easier.
- **Encryption**: Sensitive data, such as configuration files and user credentials, are securely encrypted using AES-256-GCM encryption.
- **Configuration and Authentication**: Secure loading and saving of configuration settings. User authentication is performed using bcrypt-hashed passwords for enhanced security.
- **Database Integration**: Uses SQLite to manage trades, positions, and historical data. The app interacts with a relational database via Diesel ORM for smooth data management.
- **Positions Management**: Tracks open positions, calculates the average cost, and computes both realized and unrealized PnL for each symbol.
- **Market Data Integration**: Fetches historical market data from APIs like Yahoo Finance, and supports option chain data and SEC filings.
- **IBKR Integration**: Establishes a live connection with Interactive Brokers to fetch real-time market data, place orders, and subscribe to Depth of Market (DOM) and Time & Sales (T&S) data.
- **Alert System**: Allows users to set up price-based or technical alerts that trigger user-defined callbacks. Alerts notify users about significant price movements or technical indicator changes.
- **Backtesting Engine**: Implements a backtesting system where users can test their algorithms on historical market data using simple strategies like the Simple Moving Average (SMA) strategy.
- **Real-Time Charting**: Displays real-time candlestick charts for selected symbols, updating with live market data.
- **RBAC (Role-Based Access Control)**: The application includes role-based access control, where users can have different permissions based on their roles (Admin, Trader, ReadOnly, Analyst). Administrators have full access, while traders can manage positions, bots, and run backtests.
- **Trading Account Management**: Allows users to add or remove trading accounts and manage different strategies within each account.
- **IDE (Integrated Development Environment)**: A built-in editor for algorithmic trading strategies with syntax highlighting, breakpoints, and variable watching. You can write, debug, and run algorithms in real-time.

## UI Tabs Overview
- **Market Dashboard**: Displays a real-time candlestick chart showing live price data for selected symbols.
- **Option Chain**: Displays option chain data, including available strikes, expirations, and bid/ask prices.
- **SEC Filings**: Retrieves and displays the latest SEC filings for a given symbol.
- **Backtest**: Run a backtest using historical data with strategies like SMA (Simple Moving Average).
- **DOM & T&S**: Displays Depth of Market and Time & Sales data for symbols, directly integrated with IBKR.
- **Orders**: Manage and track live orders, including placing, canceling, and updating orders.
- **ALGOs**: Manage and configure algorithmic trading systems for automated trading strategies.
- **Positions**: Tracks open positions in your account, showing current holdings, quantity, and PnL.

## Disclaimer

This project is provided for educational and research purposes only. It is not financial advice, nor an invitation to trade or invest.
The author does not guarantee the accuracy, completeness, or profitability of this trading system. Use of this code in live or paper trading environments is at your own risk.
Trading financial instruments such as stocks, options, or derivatives involves significant risk of loss and may not be suitable for all investors.
You are solely responsible for any decisions or trades you make. Before using this system, consult with a qualified financial advisor and ensure compliance with your local regulations and your broker’s terms of service.
The author is not liable for any damages, financial losses, or legal issues resulting from the use of this codebase.
