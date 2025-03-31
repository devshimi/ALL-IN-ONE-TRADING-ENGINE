# ALL IN ONE TRADING ENGINE

## Overview

The ALL In One Trading Engine is a versatile and comprehensive trading application designed to support the entire lifecycle of trading strategies, from development and testing to execution. This sophisticated tool is contained within a single Python file, making it easy to deploy and manage. It combines various functionalities like real-time charting, profit and loss tracking, Interactive Brokers (IBKR) integration, and more.

## Features

The engine offers a wide array of features to facilitate effective trading:

**Real-time Chart & Custom Candlestick**: The application allows traders to visualize market data in real-time using custom candlestick charts. This feature enables users to monitor price movements and market trends as they happen.

**PnL Tracking**: Keeping track of positions and profit & loss in real-time is crucial for traders. The engine provides a system to monitor these metrics continuously, offering insights into trading performance.

**IBKR Integration**: Direct integration with Interactive Brokers enhances the engine's capabilities by providing access to market data, order placement, Direct Order Management (DOM), and Trader Station (T&S). This seamless connection ensures that traders can execute their strategies efficiently.

**Alerts System**: The engine includes an alert system that allows users to create and manage technical or price-based alerts. This feature helps traders stay informed about important market movements and potential trading opportunities.

**SEC Filings**: To assist with fundamental analysis, the engine can fetch and display SEC filings for any given symbol. This functionality provides traders with critical information about the companies they are interested in.

**Option Chain**: The application can fetch and display option chain data using Yahoo Finance, making it easier for traders to analyze derivatives and options strategies.

**Backtester**: A built-in backtester allows users to run backtests using a simple SMA-based strategy. This tool helps traders evaluate the performance of their strategies using historical data before applying them in real-time trading.

**User Authentication**: Security is a primary concern, and the engine includes a secure user authentication system. This ensures that only authorized individuals can access the trading functionalities.

**Database Integration**: For those who need to store and retrieve trading data, the engine offers optional database integration. This feature provides a robust solution for managing large volumes of trading data.

**Extensible UI**: The user interface, built with PyQt5, is designed to be extensible and user-friendly. It includes multiple tabs, each dedicated to different aspects of trading, such as orders, bot management, positions, option chains, SEC filings, alerts, backtesting, and DOM/T&S.

## Getting Started

To get started with the ALL In One Trading Engine, you need to ensure your environment meets the prerequisites and follow a few setup steps:

**Prerequisites**: The engine requires Python 3.7 or higher. You can install the necessary packages using the provided requirements file:

```bash
pip install -r requirements.txt
```

**Configuration**: Make sure you have a `config.json` file in the same directory as the application. If this file does not exist, the application will automatically create a default configuration file. Similarly, ensure you have a `users.json` file for user authentication. If this file is missing, the application will generate it with a default admin user (`username: admin`, `password: admin`).

**Starting the Application**: Once your environment is set up, you can start the application by running:

```bash
python ultimate_pro_trading_system.py
```

## Usage

Upon starting the application, you will be greeted with a login dialog. You can log in using the default admin credentials or create new users. After logging in, the main window will be displayed, containing multiple tabs that provide access to the various functionalities of the engine.

## Disclaimer

This project is intended for educational and research purposes only. It is not financial advice and should not be construed as an invitation to trade or invest. The author does not guarantee the accuracy, completeness, or profitability of this trading software. Use of this code in live or paper trading environments is at your own risk. Trading financial instruments such as stocks, options, or derivatives involves significant risk of loss and may not be suitable for all investors. You are solely responsible for any decisions or trades you make. Before using this system, consult with a qualified financial advisor and ensure compliance with your local regulations and your broker’s terms of service. The author is not liable for any damages, financial losses, or legal issues resulting from the use of this codebase.

Enjoy exploring the ALL In One Trading Engine and use it as a platform to develop, test, and enhance your trading strategies!
