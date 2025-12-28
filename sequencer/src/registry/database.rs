use crate::BatchMetadata;

pub struct Registry {
    // TODO: Add database connection
}

impl Registry {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn store(&self, metadata: BatchMetadata) -> anyhow::Result<()> {
        // TODO: Store to database
        Ok(())
    }
}