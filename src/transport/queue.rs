
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use crate::prelude::*;
use crate::transport::transaction::WorkflowTransaction;

#[derive(Debug, Clone)]
pub struct TransactionQueue {
    pub pending : Vec<Arc<Mutex<WorkflowTransaction>>>,
    pub map : BTreeMap<Pubkey, Arc<Mutex<WorkflowTransaction>>>,
}

impl TransactionQueue {
    pub fn new() -> TransactionQueue {
        TransactionQueue {
            pending : Vec::new(),
            map : BTreeMap::new(),
        }
    }
}

