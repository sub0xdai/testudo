// @anchor exchange:common_utils:mod
// @tags infra

pub mod exchange_account;
pub mod user;

// Integration documentation and examples
#[cfg(test)]
mod integration_example;

pub use user::{User, UserError};

pub use exchange_account::{
    canonical_exchange_name, ExchangeAccount, ExchangeAccountError, ExchangeAccountFactory,
    ExchangeValidator, StandardExchangeAccountFactory, StandardExchangeValidator,
};
