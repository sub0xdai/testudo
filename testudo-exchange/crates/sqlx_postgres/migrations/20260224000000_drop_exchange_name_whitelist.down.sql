-- Restore hardcoded exchange name whitelist
ALTER TABLE exchange_accounts ADD CONSTRAINT check_exchange_name_supported
    CHECK (exchange_name IN (
        'binance', 'coinbase', 'coinbase_pro', 'kraken', 'bitstamp',
        'bitfinex', 'huobi', 'okx', 'kucoin', 'bybit'
    ));
