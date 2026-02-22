'use strict';

const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const ccxt = require('ccxt');
const { mapError } = require('./errors');

describe('mapError', () => {
  it('maps AuthenticationError to 401', () => {
    const result = mapError(new ccxt.AuthenticationError('bad key'));
    assert.equal(result.status, 401);
    assert.equal(result.body.code, 'AuthenticationError');
    assert.equal(result.body.error, 'bad key');
  });

  it('maps InsufficientFunds to 402', () => {
    const result = mapError(new ccxt.InsufficientFunds('no funds'));
    assert.equal(result.status, 402);
    assert.equal(result.body.code, 'InsufficientFunds');
  });

  it('maps OrderNotFound to 404', () => {
    const result = mapError(new ccxt.OrderNotFound('order gone'));
    assert.equal(result.status, 404);
    assert.equal(result.body.code, 'OrderNotFound');
  });

  it('maps RateLimitExceeded to 429', () => {
    const result = mapError(new ccxt.RateLimitExceeded('slow down'));
    assert.equal(result.status, 429);
    assert.equal(result.body.code, 'RateLimitExceeded');
  });

  it('maps ExchangeNotAvailable to 503', () => {
    const result = mapError(new ccxt.ExchangeNotAvailable('down'));
    assert.equal(result.status, 503);
    assert.equal(result.body.code, 'ExchangeNotAvailable');
  });

  it('maps NetworkError to 502', () => {
    const result = mapError(new ccxt.NetworkError('timeout'));
    assert.equal(result.status, 502);
    assert.equal(result.body.code, 'NetworkError');
  });

  it('maps other ccxt errors to 500', () => {
    const result = mapError(new ccxt.ExchangeError('generic exchange error'));
    assert.equal(result.status, 500);
    assert.equal(result.body.code, 'ExchangeError');
  });

  it('maps non-ccxt errors to 500 with UnknownError code', () => {
    const result = mapError(new Error('something broke'));
    assert.equal(result.status, 500);
    assert.equal(result.body.code, 'UnknownError');
    assert.equal(result.body.error, 'something broke');
  });

  it('returns correct body format', () => {
    const result = mapError(new Error('test'));
    assert.ok('status' in result);
    assert.ok('body' in result);
    assert.ok('error' in result.body);
    assert.ok('code' in result.body);
    assert.equal(typeof result.status, 'number');
    assert.equal(typeof result.body.error, 'string');
    assert.equal(typeof result.body.code, 'string');
  });
});
