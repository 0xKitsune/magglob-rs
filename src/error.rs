use thiserror::Error;
use tungstenite::error;

#[derive(Error, Debug)]
pub enum OrderBookError {
    #[error("Reqwest error")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Tungstenite error")]
    TungsteniteError(#[from] tungstenite::Error),
    #[error("HTTP error")]
    HTTPError(String),
}
