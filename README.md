# Ultimate Trading App (Real-Time Chart + PnL) - Pro Edition

## Introduction
The Ultimate Trading App is designed for professional and advanced traders, offering real-time charting, PnL tracking, market data integration, and algorithmic trading features. The application leverages IBKR for live market data and order management, along with various other APIs for enhanced functionality.

## Getting Started

### Prerequisites
- Python 3.8 or higher
- Required Python packages (listed in `requirements.txt`)

### Installation
1. Clone the repo:
   ```sh
   git clone https://github.com/devshimi/ultimate-trading-app.git
   cd ultimate-trading-app
   ```
2. Install the required packages:
   ```sh
   pip install -r requirements.txt
   ```

### Running the Application
1. Start the application:
   ```sh
   python main.py
   ```

## Features Overview

### Custom Exceptions
- **ConfigError**: Raised for critical configuration issues.
- **AuthenticationError**: Raised for authentication failures.
- **IBKRConnectionError**: Raised for IBKR connection issues.
- **DatabaseError**: Raised for critical database issues.

### Logger Setup
- Configures a rotating file logger and console stream handler for detailed logging.

### Encryption
- Uses `cryptography.fernet` for encrypting sensitive data like configuration files and user credentials.

### Configuration
- Loads and saves configuration settings securely.

### Authentication
- Manages user authentication with bcrypt hashing, supporting admin user management.

### Database
- Integrates SQLite with SQLAlchemy for managing trades and positions.

### Positions Management
- Tracks open positions, calculates average cost, and realized/unrealized PnL.

### Market Data
- Fetches historical data from Yahoo Finance and SEC filings, with support for option chain data.

### IBKR Integration
- Manages IBKR connection, live market data fetching, order placement, and DOM/T&S subscriptions.

### Alert System
- Manages technical or price-based alerts, triggering user-defined callbacks.

### Backtester
- Implements a simple SMA-based strategy for backtesting on historical data.

### Real-Time Charting
- Displays real-time candlestick charts using PyQtGraph.

### UI Tabs Overview
- **Market Dashboard**: Real-time candlestick chart for selected symbols.
- **Option Chain**: Fetch and display option chain data from Yahoo Finance.
- **SEC Filings**: Retrieve and display the latest SEC filings for a given symbol.
- **Alerts**: Set and manage price-based alerts.
- **Backtest**: Run a simple SMA-based backtest on historical data.
- **DOM & T&S**: Display Depth of Market and Time & Sales data for a selected symbol (IBKR only).
- **Orders**: View and manage live orders.
- **Bots**: Manage algorithmic trading bots.
- **Positions**: Track open positions and their PnL.

## Disclaimer
This project is provided for educational and research purposes only. It is not financial advice, nor an invitation to trade or invest.
The author does not guarantee the accuracy, completeness, or profitability of this trading software. Use of this code in live or paper trading environments is at your own risk.
Trading financial instruments such as stocks, options, or derivatives involves significant risk of loss and may not be suitable for all investors. You are solely responsible for any decisions or trades you make.
Before using this system, consult with a qualified financial advisor and ensure compliance with your local regulations and your broker’s terms of service.
The author is not liable for any damages, financial losses, or legal issues resulting from the use of this codebase.
