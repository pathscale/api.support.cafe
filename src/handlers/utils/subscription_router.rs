use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

use endpoint_libs::libs::toolbox::{RequestContext, Toolbox};
use endpoint_libs::libs::ws::SubscriptionManager;
use futures::{Stream, StreamExt};
use serde::Serialize;
use tokio::sync::RwLock;

use super::receiver::Receiver;
use super::routing_message::RoutingMessage;

pub struct SubscriptionRouter<K, M> {
    event_manager: Arc<RwLock<SubscriptionManager<(), K>>>,
    _task_handle: tokio::task::JoinHandle<()>,
    _marker: PhantomData<M>,
}

impl<K, M> SubscriptionRouter<K, M>
where
    K: Clone + Eq + Hash + Sync + Send + 'static,
    M: Clone + Serialize + Send + 'static,
{
    pub fn new<S>(stream_code: u32, stream: S, toolbox: Arc<Toolbox>) -> Self
    where
        S: Stream<Item = RoutingMessage<K, M>> + Send + Unpin + 'static,
    {
        let event_manager = Arc::new(RwLock::new(SubscriptionManager::new(stream_code)));
        let event_manager_clone = event_manager.clone();
        let toolbox_clone = toolbox.clone();

        let task_handle = tokio::spawn(async move {
            let mut stream = stream;
            while let Some(msg) = stream.next().await {
                match msg.receiver {
                    Receiver::Concrete(key) => {
                        event_manager_clone
                            .write()
                            .await
                            .publish_to_key(&toolbox_clone, &key, &msg.payload);
                    }
                    Receiver::ConcreteMulti(keys) => {
                        let mut manager = event_manager_clone.write().await;
                        for key in keys {
                            manager.publish_to_key(&toolbox_clone, &key, &msg.payload.clone());
                        }
                    }
                    Receiver::All => {
                        event_manager_clone
                            .write()
                            .await
                            .publish_to_all(&toolbox_clone, &msg.payload);
                    }
                }
            }
        });

        Self {
            event_manager,
            _task_handle: task_handle,
            _marker: PhantomData,
        }
    }

    pub async fn subscribe(&self, ctx: RequestContext, keys: Vec<K>) {
        self.event_manager
            .write()
            .await
            .subscribe_with_keys(ctx, keys, (), |_| {});
    }

    pub async fn unsubscribe(&self, connection_id: u32) {
        self.event_manager.write().await.unsubscribe(connection_id);
    }
}
