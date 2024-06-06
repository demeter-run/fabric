use anyhow::Result;

#[derive(Debug, Clone)]
pub enum Event {
    NamespaceCreate(NamespaceCreate),
}

#[derive(Debug, Clone)]
pub struct NamespaceCreate {
    pub name: String,
    pub slug: String,
}

#[async_trait::async_trait]
pub trait EventBridge {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
