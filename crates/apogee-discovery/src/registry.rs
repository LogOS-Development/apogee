//! Active service registry and schema versioning.

use std::collections::HashMap;

/// Unique identifier for a dynamic service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ServiceId(pub u64);

/// Registry of active dynamic services.
#[derive(Debug, Clone, Default)]
pub struct ServiceRegistry {
    services: HashMap<ServiceId, ServiceSchema>,
}

/// Schema for a dynamic service's inputs and outputs.
#[derive(Debug, Clone, Default)]
pub struct ServiceSchema {
    pub id: ServiceId,
    pub name: String,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    pub fn register(&mut self, schema: ServiceSchema) {
        self.services.insert(schema.id, schema);
    }

    pub fn unregister(&mut self, id: ServiceId) {
        self.services.remove(&id);
    }
}
