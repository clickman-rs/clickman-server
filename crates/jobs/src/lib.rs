use chrono::{DateTime, Utc};
use clickman_destinations::Dispatcher;
use clickman_storage::{Result, Storage};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct Jobs {
    storage: Storage,
    dispatcher: Arc<dyn Dispatcher>,
}

impl Jobs {
    pub fn new(storage: Storage, dispatcher: Arc<dyn Dispatcher>) -> Self {
        Self {
            storage,
            dispatcher,
        }
    }
    pub async fn rotate(&self, project: Uuid, now: DateTime<Utc>) -> Result<u64> {
        let count = self.storage.rotate(project, now).await?;
        self.storage.prune(project, now).await?;
        self.storage.refresh_reports(project, now).await?;
        Ok(count)
    }
    pub async fn deliver(&self, project: Uuid, now: DateTime<Utc>) -> Result<u64> {
        let mut delivered = 0;
        for destination in self.storage.destinations(project).await? {
            // Bound each tick so one busy project cannot starve the others.
            for _ in 0..10 {
                let Some(page) = self
                    .storage
                    .delivery_page(project, destination.id, now, 2000)
                    .await?
                else {
                    break;
                };
                let count = page.events.len() as u64;
                match self
                    .dispatcher
                    .deliver(&page.destination, &page.events)
                    .await
                {
                    Ok(()) => {
                        page.complete(Utc::now()).await?;
                        delivered += count;
                    }
                    Err(error) => {
                        tracing::warn!(%project,destination=%destination.id,%error,"destination will be retried");
                        page.retry(Utc::now(), &error.to_string()).await?;
                        break;
                    }
                }
                if count < 2000 {
                    break;
                }
            }
        }
        Ok(delivered)
    }
    /// Every tick re-derives due work from PostgreSQL. Restarting the scheduler
    /// cannot lose an event, an unfinished rotation, or a destination retry.
    pub async fn tick(&self) -> Result<()> {
        for project in self.storage.projects().await? {
            let now = Utc::now();
            if let Err(error) = self.rotate(project.id, now).await {
                tracing::error!(project=%project.id,%error,"rotation failed");
            }
            if let Err(error) = self.deliver(project.id, now).await {
                tracing::error!(project=%project.id,%error,"delivery failed");
            }
        }
        Ok(())
    }
}
