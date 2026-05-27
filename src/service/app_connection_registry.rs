use std::collections::HashMap;

use tokio::sync::RwLock;

use crate::id_types::AppPublicId;

pub type ConnectionId = u32;

// TODO: remove when elibs will be capable to have xustome context.
pub struct AppConnectionRegistry {
    connections: RwLock<HashMap<ConnectionId, AppPublicId>>,
}

impl AppConnectionRegistry {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register(&self, conn_id: ConnectionId, app_id: AppPublicId) {
        self.connections.write().await.insert(conn_id, app_id);
    }

    pub async fn unregister(&self, conn_id: ConnectionId) {
        self.connections.write().await.remove(&conn_id);
    }

    pub async fn get(&self, conn_id: ConnectionId) -> Option<AppPublicId> {
        self.connections.read().await.get(&conn_id).copied()
    }
}

impl Default for AppConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
