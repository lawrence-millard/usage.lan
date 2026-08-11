pub struct ModelPrice {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
}

/// Ordered list of (substring, price). Longer, more specific patterns come first.
const PRICES: &[(&str, f64, f64)] = &[
    ("gpt-5.6", 1.25, 10.0),
    ("gpt-5.5", 1.25, 10.0),
    ("gpt-5.4", 1.25, 10.0),
    ("gpt-5.3", 1.25, 10.0),
    ("gpt-5.2", 1.25, 10.0),
    ("gpt-5.1-mini", 0.25, 2.0),
    ("gpt-5.1", 1.25, 10.0),
    ("gpt-5-mini", 0.25, 2.0),
    ("gpt-5", 1.25, 10.0),
    ("gpt-4o-mini", 0.15, 0.6),
    ("gpt-4o", 2.5, 10.0),
    ("gpt-4.1-mini", 0.4, 1.6),
    ("gpt-4.1", 2.0, 8.0),
    ("gpt-4", 30.0, 60.0),
    ("o4-mini", 1.1, 4.4),
    ("o3-mini", 1.1, 4.4),
    ("o3", 2.0, 8.0),
    ("o1", 15.0, 60.0),
    ("claude-opus-4-6", 5.0, 25.0),
    ("claude-opus-4-5", 5.0, 25.0),
    ("claude-opus-4-1", 5.0, 25.0),
    ("claude-opus-4", 5.0, 25.0),
    ("claude-sonnet-4-6", 3.0, 15.0),
    ("claude-sonnet-4-5", 3.0, 15.0),
    ("claude-sonnet-4-1", 3.0, 15.0),
    ("claude-sonnet-4", 3.0, 15.0),
    ("claude-haiku-4-5", 1.0, 5.0),
    ("claude-haiku-4", 1.0, 5.0),
    ("deepseek-v4-flash", 0.15, 0.45),
    ("deepseek-v4", 0.4, 0.8),
    ("deepseek-v3.2", 0.28, 0.42),
    ("deepseek-v3.1", 0.28, 0.42),
    ("deepseek-v3", 0.27, 1.1),
    ("deepseek-r1", 0.55, 2.19),
    ("deepseek-chat", 0.27, 1.1),
    ("gemini-3-pro", 2.0, 12.0),
    ("gemini-3-flash", 0.3, 2.5),
    ("gemini-2.5-pro", 1.25, 10.0),
    ("gemini-2.5-flash", 0.3, 2.5),
    ("gemini-2.0-flash", 0.1, 0.4),
    ("llama-4-maverick", 0.25, 0.75),
    ("llama-4-scout", 0.15, 0.6),
    ("llama-3.3", 0.25, 0.75),
    ("grok-4", 3.0, 15.0),
    ("grok-3", 3.0, 15.0),
    ("mistral-large", 2.0, 6.0),
    ("mistral-small", 0.1, 0.3),
    ("qwen-3-max", 1.2, 6.0),
    ("qwen-2.5", 0.4, 1.2),
    ("kimi-k2", 0.6, 2.5),
];

pub fn price_for(model: &str, provider: &str) -> Option<ModelPrice> {
    let lower = model.to_lowercase();
    let provider_lower = provider.to_lowercase();

    if lower.contains("free") || provider_lower.contains("free") {
        return None;
    }

    for (pattern, i, o) in PRICES {
        if lower.contains(pattern) {
            return Some(ModelPrice {
                input_per_mtok: *i,
                output_per_mtok: *o,
            });
        }
    }
    None
}

/// Estimate cost in dollars from token counts. Cache reads are charged at a
/// discounted input rate, cache writes at a premium input rate.
pub fn estimate_cost(model: &str, provider: &str, rec: &crate::models::UsageRecord) -> f64 {
    let Some(price) = price_for(model, provider) else {
        return 0.0;
    };
    let pi = price.input_per_mtok;
    let po = price.output_per_mtok;
    (rec.tokens_input as f64 * pi
        + rec.tokens_cache_read as f64 * pi * 0.1
        + rec.tokens_cache_write as f64 * pi * 1.25
        + (rec.tokens_output + rec.tokens_reasoning) as f64 * po)
        / 1_000_000.0
}
