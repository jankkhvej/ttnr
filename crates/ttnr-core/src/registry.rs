//! [`SourceRegistry`]: the one place sources are registered (SPEC F1.2).

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::resolve::{Mode, ResolveCtx, ResolveError, resolve_chain};
use crate::{Source, Station, StreamDescriptor};

/// All known sources, by id.
#[derive(Default)]
pub struct SourceRegistry {
    sources: BTreeMap<&'static str, Arc<dyn Source>>,
}

impl SourceRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a source. A second source with the same id replaces the first
    /// (and is logged): ids must be unique.
    pub fn register(&mut self, source: Arc<dyn Source>) {
        let id = source.id();
        if self.sources.insert(id, source).is_some() {
            tracing::warn!(id, "source registered twice; keeping the last one");
        }
    }

    /// The source with `id`.
    pub fn get(&self, id: &str) -> Option<&Arc<dyn Source>> {
        self.sources.get(id)
    }

    /// Registered ids, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.sources.keys().copied()
    }

    /// Resolve `st` with its source, cache first.
    pub async fn resolve(
        &self,
        st: &Station,
        ctx: &ResolveCtx,
    ) -> Result<StreamDescriptor, ResolveError> {
        self.run(st, ctx, Mode::Normal).await
    }

    /// Re-discover `st`, bypassing (and replacing) the cache.
    pub async fn refresh(
        &self,
        st: &Station,
        ctx: &ResolveCtx,
    ) -> Result<StreamDescriptor, ResolveError> {
        self.run(st, ctx, Mode::Refresh).await
    }

    async fn run(
        &self,
        st: &Station,
        ctx: &ResolveCtx,
        mode: Mode,
    ) -> Result<StreamDescriptor, ResolveError> {
        let source = self
            .get(&st.source)
            .ok_or_else(|| ResolveError::UnknownSource(st.source.clone()))?;
        resolve_chain(source.as_ref(), st, ctx, mode).await
    }
}

impl std::fmt::Debug for SourceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.sources.keys()).finish()
    }
}
