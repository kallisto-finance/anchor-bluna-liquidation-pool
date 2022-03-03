use cosmwasm_std::{ConversionOverflowError, OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Standard Error")]
    Std(#[from] StdError),

    #[error("Overflow")]
    OverflowError(#[from] OverflowError),

    #[error("Conversion Overflow")]
    ConversionOverflowError(#[from] ConversionOverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Wrong Coin Data")]
    CoinError {},

    #[error("Wrong Coin Count")]
    CoinCountError {},

    #[error("Insufficient USD")]
    InsufficientUSD {},

    #[error("Insufficient Balance")]
    InsufficientBalance {},

    #[error("Invalidate Input")]
    Invalidate {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
