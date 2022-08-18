use std::sync::Arc;
use std::sync::Mutex;
use manual_future::{ManualFuture, ManualFutureCompleter};
use ahash::AHashMap;
use std::hash::Hash;
use std::cmp::Eq;
use std::fmt::Display;
use crate::result::Result;

pub type LookupResult<T> = Result<Option<T>>;

pub enum RequestType<T : Unpin> {
    New(ManualFuture<LookupResult<T>>),
    Pending(ManualFuture<LookupResult<T>>)
}

pub struct LookupHandler<K, T : Unpin> {
    pub pending : Arc<Mutex<AHashMap<K,Vec<ManualFutureCompleter<LookupResult<T>>>>>>
}

impl<K,T> LookupHandler<K,T> where T : Unpin + Clone, K : Clone + Eq + Hash + Display {
    pub fn new() -> Self {
        LookupHandler {
            pending : Arc::new(Mutex::new(AHashMap::new()))
        }
    }

    pub fn queue(&self, key: &K) -> RequestType<T> {

        let mut pending = self.pending.lock().unwrap();
        let (future, completer) = ManualFuture::<LookupResult<T>>::new();

        if let Some(list) = pending.get_mut(&key) {
            list.push(completer);
            RequestType::Pending(future)
        } else {
            let mut list = Vec::new();
            list.push(completer);
            pending.insert(key.clone(),list);
            RequestType::New(future)
        }
    }

    pub async fn complete(&self, key : &K, result : LookupResult<T>) {
        let mut pending = self.pending.lock().unwrap();

        if let Some(list) = pending.remove(&key) {
            for completer in list {
                completer.complete(result.clone()).await;
            }
        } else {
            panic!("Lookup handler failure while processing account {}", key)
        }
    }
}

#[cfg(not(target_arch = "bpf"))]
#[cfg(any(test, feature="test"))]
mod tests {
    use std::time::Duration;

    use super::*;
    use futures::join;
    use async_std::task::sleep;
    use workflow_log::log_trace;
    use wasm_bindgen::prelude::*;

    #[derive(Debug, Eq, PartialEq)]
    enum RequestTypeTest {
        New = 0,
        Pending = 1,
    }

    struct LookupHandlerTest {
        pub lookup_handler : LookupHandler<u32,u32>,
        pub map : Arc<Mutex<AHashMap<u32,u32>>>,
        pub request_types : Arc<Mutex<Vec<RequestTypeTest>>>,
    }

    impl LookupHandlerTest {

        pub fn new() -> Self {
            Self {
                lookup_handler : LookupHandler::new(),
                map : Arc::new(Mutex::new(AHashMap::new())),
                request_types : Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn insert(self : &Arc<Self>, key : u32, value : u32) -> Result<()> {
            let mut map = self.map.lock()?;
            map.insert(key, value);
            Ok(())
        }

        pub async fn lookup_remote_impl(self : &Arc<Self>, key:&u32) -> Result<Option<u32>> {
            log_trace!("[lh] lookup sleep...");
            sleep(Duration::from_millis(100)).await;
            log_trace!("[lh] lookup wake...");
            let map = self.map.lock()?;
            Ok(map.get(&key).cloned())
        }

        pub async fn lookup_handler_request(self : &Arc<Self>, key:&u32) -> Result<Option<u32>> {

            // let request_type = self.clone().lookup_handler.queue(key);
            let request_type = self.lookup_handler.queue(key);
            match request_type {
                RequestType::New(future) => {
                    self.request_types.lock().unwrap().push(RequestTypeTest::New);
                    log_trace!("[lh] new request");
                    let response = self.lookup_remote_impl(key).await;
                    log_trace!("[lh] completing initial request");
                    self.lookup_handler.complete(key, response).await;
                    future.await
                },
                RequestType::Pending(future) => {
                    self.request_types.lock().unwrap().push(RequestTypeTest::Pending);
                    log_trace!("[lh] pending request");
                    future.await
                }
            }
        }
    }

    
    
    #[wasm_bindgen]
    pub async fn lookup_handler_test() -> Result<()> {

        let lht = Arc::new(LookupHandlerTest::new());
        lht.insert(0xc0fee,0xdecaf)?;
        
        let v0 = lht.lookup_handler_request(&0xc0fee);
        let v1 = lht.lookup_handler_request(&0xc0fee);
        let v2 = lht.lookup_handler_request(&0xc0fee);
        let f = join!(v0, v1, v2);

        log_trace!("[lh] results: {:?}", f);
        let f = (f.0.unwrap().unwrap(), f.1.unwrap().unwrap(), f.2.unwrap().unwrap()); 
        assert_eq!(f,(0xdecaf,0xdecaf,0xdecaf));

        let request_types = lht.request_types.lock().unwrap();
        log_trace!("[lh] request types: {:?}", request_types);
        assert_eq!(request_types[..], [RequestTypeTest::New,RequestTypeTest::Pending,RequestTypeTest::Pending]);
        log_trace!("all looks good ... 😎");

        Ok(())
    }

    #[cfg(not(any(target_arch = "wasm32", target_arch = "bpf")))]
    #[cfg(test)]
    mod tests {
        use super::*;

        #[async_std::test]
        pub async fn lookup_handler_test() -> Result<()> {
            super::lookup_handler_test().await
        }
    }
}
