use crate::Agent;

/// Supplies model services to a crawler independently of router state.
pub trait AgentCrawlerExt: Sized {
    /// Registers the agent used by model extractors and collectors.
    #[must_use]
    fn with_agent(self, agent: Agent) -> Self;
}

impl<B, W> AgentCrawlerExt for spire_core::crawler::Crawler<B, W> {
    fn with_agent(self, agent: Agent) -> Self {
        self.with_extension(agent)
    }
}
