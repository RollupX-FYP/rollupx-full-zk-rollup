use crate::config::L1Config;

pub struct L1Listener {
    config: L1Config,
}

impl L1Listener {
    pub fn new(config: L1Config) -> Self {
        Self { config }
    }
    
    pub async fn start(&self) -> anyhow::Result<()> {
        // TODO: Connect to L1 and listen for events
        Ok(())
    }
}