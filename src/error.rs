use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiSpeedError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Failed to parse streaming response: {0}")]
    #[allow(dead_code)]
    ParseError(String),

    #[error("API returned an error: {0}")]
    #[allow(dead_code)]
    ApiError(String),

    #[error("No tokens received")]
    NoTokens,
}

impl ApiSpeedError {
    pub fn user_message(&self) -> String {
        match self {
            ApiSpeedError::RequestError(e) => {
                if e.is_timeout() {
                    "Request timed out. Check your network or try a smaller prompt.".to_string()
                } else if e.is_connect() {
                    "Failed to connect. Check the URL is correct and the server is reachable."
                        .to_string()
                } else {
                    format!(
                        "Request failed: {}. Check your API key and network connection.",
                        e
                    )
                }
            }
            ApiSpeedError::ParseError(s) => format!("Failed to parse response: {}", s),
            ApiSpeedError::ApiError(s) => s.clone(),
            ApiSpeedError::NoTokens => {
                "No tokens received from the API. The model may not support streaming.".to_string()
            }
        }
    }
}
