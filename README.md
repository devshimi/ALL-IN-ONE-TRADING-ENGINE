# ALL IN ONE TRADING ENGINE

## Overview

The ALL IN ONE TRADING ENGINE is a comprehensive trading platform designed for educational and research purposes. It features real-time market data, automated trading strategies, position management, and various analytical tools. The app integrates with Interactive Brokers (IBKR) for live trading and market data, as well as Yahoo Finance for historical data.

## Features

1. **Custom Exceptions**: Defined for handling various critical errors like configuration issues, authentication failures, and database connection problems.

2. **Logger Setup**: Configures a rotating file logger along with a console stream handler for logging application activities.

3. **Encryption**: Utilizes Fernet encryption for securely storing sensitive information such as configuration and user credentials.

4. **Configuration Management**: Loads, encrypts, and decrypts the configuration settings from a JSON file.

5. **Authentication**: Manages user authentication using bcrypt for password hashing and verification.

6. **Database Integration**: Supports SQLite for storing trade and position records, using SQLAlchemy for ORM.

7. **Positions Management**: Tracks open positions, calculates average costs and realized PnL, and logs positions to the database.

8. **Market Data Interface**: Fetches market data from Yahoo Finance and SEC filings, and integrates with IBKR for live data.

9. **IBKR Integration**: Manages IBKR connection for real-time market data, placing/canceling orders, and handling events.

10. **Alert System**: Manages technical or price-based alerts and triggers user-defined callbacks when conditions are met.

11. **Backtester**: Provides a simple backtesting framework using a moving average cross strategy.

12. **Candlestick Visualization**: Custom PyQtGraph item for rendering candlestick charts.

13. **Chart Data Management**: Maintains rolling candlestick data for real-time updates.

14. **Login Dialog**: Provides a login interface with an option to skip IBKR for offline mode.

15. **Strategy Engine**: Placeholder for advanced trading strategies.

16. **UI Tabs**: Various tabs for managing orders, bots, positions, option chains, SEC filings, alerts, backtesting, and DOM & T&S.

17. **Main Window**: The main application window that integrates all features and functionalities.

18. **Entry Point**: The main function that launches the login dialog and the main application window.

## Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/all-in-one-trading-engine.git
   cd all-in-one-trading-engine
   ```

2. Install the required dependencies:
   ```bash
   pip install -r requirements.txt
   ```

3. Ensure you have PyQt5 installed. If not, install it using:
   ```bash
   pip install PyQt5
   ```

4. Set up your configuration and user files:
   - `config.json`: Configuration settings for IBKR and API keys.
   - `users.json`: User credentials for authentication.

5. Run the application:
   ```bash
   python main.py
   ```

## Usage

- **Login**: Use the login dialog to authenticate or skip IBKR for offline mode.
- **Tabs**: Navigate through various tabs to manage orders, bots, positions, option chains, SEC filings, alerts, backtesting, and DOM & T&S.
- **Real-Time Data**: The application fetches real-time market data and updates the charts and positions accordingly.
- **Alerts**: Set price-based alerts and get notified when conditions are met.
- **Backtesting**: Run simple moving average cross strategy backtests using historical data.


## Disclaimer

This project is provided for educational and research purposes only. It is not financial advice, nor an invitation to trade or invest. The author does not guarantee the accuracy, completeness, or profitability of this trading software. Use of this code in live or paper trading environments is at your own risk.

Trading financial instruments such as stocks, options, or derivatives involves significant risk of loss and may not be suitable for all investors. You are solely responsible for any decisions or trades you make.

Before using this system, consult with a qualified financial advisor and ensure compliance with your local regulations and your broker’s terms of service.

The author is not liable for any damages, financial losses, or legal issues resulting from the use of this codebase.
