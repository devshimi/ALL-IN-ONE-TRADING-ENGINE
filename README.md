# ALL IN ONE TRADING ENGINE 

## Overview
The ALL In One Trading Engine is a sophisticated, single-file trading application equipped with a range of features to support the development, testing, and execution of trading strategies. It integrates multiple functionalities, including real-time charting, PnL tracking, IBKR integration, and more, all within a single Python file.

## Features
- **Real-time Chart & Custom Candlestick**: Visualize market data in real-time with custom candlestick charts.
- **PnL Tracking**: Track positions and profit & loss in real-time.
- **IBKR Integration**: Direct integration with Interactive Brokers for market data, order placement, DOM, and T&S.
- **Alerts System**: Create and manage technical or price-based alerts.
- **SEC Filings**: Fetch and display SEC filings for a given symbol.
- **Option Chain**: Fetch and display option chain data using Yahoo Finance.
- **Backtester**: Run backtests with a simple SMA-based strategy.
- **User Authentication**: Secure user authentication system.
- **Database**: Optional database integration for storing and retrieving trading data.
- **Extensible UI**: PyQt5-based user interface with multiple tabs for different functionalities.

## Sections
1. Custom Exceptions
2. Logger Setup (Rotating + Console)
3. Encryption (Fernet)
4. Configuration
5. Authentication
6. Database
7. Positions
8. Market Data
9. IBKR Integration
10. Alert System
11. Backtester
12. Custom CandlestickItem
13. ChartData
14. LoginDialog
15. StrategyEngine
16. UI Tabs:
    - OrdersTab
    - BotManagementTab
    - PositionsTab
    - OptionChainTab
    - SecFilingsTab
    - AlertsTab
    - BacktestTab
    - DomTsTab
17. MainWindow (All Tabs + Real-time Thread)
18. main() – Entry Point

## Getting Started

### Prerequisites
- Python 3.7+
- Install required packages:
    ```bash
    pip install -r requirements.txt
    ```

### Running the Application
1. **Configuration**:
   - Ensure you have a `config.json` file in the same directory. If it does not exist, the application will create a default configuration file.
   - Ensure you have a `users.json` file for user authentication. If it does not exist, the application will create a default admin user (`username: admin`, `password: admin`).

2. **Starting the Application**:
    ```bash
    python ultimate_pro_trading_system.py
    ```

### Usage
- **Login**: The application starts with a login dialog. You can login using the default admin credentials or create new users.
- **Main Window**: After logging in, the main window is displayed with multiple tabs for different functionalities.

## Disclaimer
This project is provided for educational and research purposes only. It is not financial advice, nor an invitation to trade or invest. The author does not guarantee the accuracy, completeness, or profitability of this trading software. Use of this code in live or paper trading environments is at your own risk. Trading financial instruments such as stocks, options, or derivatives involves significant risk of loss and may not be suitable for all investors. You are solely responsible for any decisions or trades you make. Before using this system, consult with a qualified financial advisor and ensure compliance with your local regulations and your broker’s terms of service. The author is not liable for any damages, financial losses, or legal issues resulting from the use of this codebase.
