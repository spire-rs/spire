use crate::{Error, Result, Usage};

/// Token and cost limits for model calls.
///
/// Limits use settled provider usage. Already-running calls can cross a limit;
/// no new calls start once it has been reached.
#[derive(Debug, Clone, Copy, Default)]
pub struct Budget {
    pub(crate) tokens: Option<u64>,
    pub(crate) cost: Option<f64>,
}

impl Budget {
    /// Caps total reported input plus output tokens.
    #[must_use]
    pub const fn tokens(max: u64) -> Self {
        Self {
            tokens: Some(max),
            cost: None,
        }
    }

    /// Also caps reported or fallback-priced cost, in USD.
    ///
    /// Non-finite or negative values fail before any provider call.
    #[must_use]
    pub const fn max_cost(mut self, usd: f64) -> Self {
        self.cost = Some(usd);
        self
    }

    pub(crate) fn validate(self) -> Result<()> {
        if self.cost.is_some_and(|usd| !usd.is_finite() || usd < 0.0) {
            return Err(Error::Configuration(
                "cost budget must be finite and nonnegative".into(),
            ));
        }
        Ok(())
    }
}

/// Fallback USD prices per million input and output tokens.
///
/// Provider costs always take precedence. Zero prices explicitly permit free
/// local models.
#[derive(Debug, Clone, Copy)]
pub struct Pricing {
    input: f64,
    output: f64,
}

impl Pricing {
    /// Sets fallback prices per million tokens.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or negative prices.
    pub fn new(input: f64, output: f64) -> Result<Self> {
        if [input, output]
            .iter()
            .any(|price| !price.is_finite() || *price < 0.0)
        {
            return Err(Error::Configuration(
                "prices must be finite and nonnegative".into(),
            ));
        }
        Ok(Self { input, output })
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "USD token pricing uses floating-point provider prices"
    )]
    pub(crate) fn cost(self, usage: Usage) -> Option<f64> {
        Some(
            (usage.input_tokens? as f64 * self.input + usage.output_tokens? as f64 * self.output)
                / 1_000_000.0,
        )
    }
}
