/// Resource usage reported for one model call.
///
/// Missing metrics remain unknown, not zero; a budget requiring them fails
/// rather than ignoring the call.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    /// Input tokens, when reported.
    pub input_tokens: Option<u64>,
    /// Output tokens, including reasoning, when reported.
    pub output_tokens: Option<u64>,
}

impl Usage {
    /// Creates reported input/output token usage.
    #[must_use]
    pub const fn tokens(input: u64, output: u64) -> Self {
        Self {
            input_tokens: Some(input),
            output_tokens: Some(output),
        }
    }
}

/// Shared model usage across agent clones.
///
/// Token totals contain known usage only; unknown metrics are explicitly
/// counted separately.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UsageTotals {
    /// Provider calls started, including calls that fail or are cancelled.
    pub calls: u64,
    /// Known input tokens.
    pub input_tokens: u64,
    /// Known output tokens.
    pub output_tokens: u64,
    /// Calls whose complete input/output token counts were unavailable.
    pub unmetered_calls: u64,
    /// Answers served from the local answer cache without a provider call.
    pub cache_hits: u64,
}
