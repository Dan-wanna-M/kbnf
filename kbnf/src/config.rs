use ebnf::{grammar::CompressionConfig, regex::FiniteStateAutomatonConfig};

use crate::engine_base::EngineConfig;
#[derive(Debug, Clone)]
pub struct Config {
    pub regex_config: FiniteStateAutomatonConfig,
    pub compression_config: CompressionConfig,
    pub engine_config: EngineConfig,
    pub excepted_config: FiniteStateAutomatonConfig,
}
