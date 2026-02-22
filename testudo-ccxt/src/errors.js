'use strict';

const ccxt = require('ccxt');

/**
 * Map a CCXT exception (or any error) to an HTTP status code and response body.
 * @param {Error} err
 * @returns {{ status: number, body: { error: string, code: string } }}
 */
function mapError(err) {
  if (err instanceof ccxt.AuthenticationError) {
    return { status: 401, body: { error: err.message, code: 'AuthenticationError' } };
  }
  if (err instanceof ccxt.InsufficientFunds) {
    return { status: 402, body: { error: err.message, code: 'InsufficientFunds' } };
  }
  if (err instanceof ccxt.OrderNotFound) {
    return { status: 404, body: { error: err.message, code: 'OrderNotFound' } };
  }
  if (err instanceof ccxt.RateLimitExceeded) {
    return { status: 429, body: { error: err.message, code: 'RateLimitExceeded' } };
  }
  if (err instanceof ccxt.ExchangeNotAvailable) {
    return { status: 503, body: { error: err.message, code: 'ExchangeNotAvailable' } };
  }
  if (err instanceof ccxt.NetworkError) {
    return { status: 502, body: { error: err.message, code: 'NetworkError' } };
  }
  if (err instanceof ccxt.BaseError) {
    return { status: 500, body: { error: err.message, code: err.constructor.name } };
  }

  return { status: 500, body: { error: err.message || 'Unknown error', code: 'UnknownError' } };
}

module.exports = { mapError };
