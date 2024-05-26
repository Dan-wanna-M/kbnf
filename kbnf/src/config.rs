use ebnf::{grammar::CompressionConfig, regex::FiniteStateAutomatonConfig};

pub struct Config
{
    pub regex_config: FiniteStateAutomatonConfig,
    pub compression_config: CompressionConfig,
}