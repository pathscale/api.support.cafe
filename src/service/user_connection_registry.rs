use std::collections::HashMap;

use honey_id_types::id_entities::UserPublicId;
use tokio::sync::RwLock;

pub type ConnectionId = u32;

// TODO: remove when elibs will be capable to have xustome context.
pub struct UserConnectionRegistry {
    connections: RwLock<HashMap<ConnectionId, UserPublicId>>,
}

impl UserConnectionRegistry {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register(&self, conn_id: ConnectionId, user_pub_id: UserPublicId) {
        self.connections.write().await.insert(conn_id, user_pub_id);
    }

    pub async fn unregister(&self, conn_id: ConnectionId) {
        self.connections.write().await.remove(&conn_id);
    }

    pub async fn get(&self, conn_id: ConnectionId) -> Option<UserPublicId> {
        self.connections.read().await.get(&conn_id).copied()
    }
}

impl Default for UserConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
