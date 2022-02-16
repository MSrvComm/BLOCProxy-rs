use std::{
    collections::HashMap,
    sync::{Arc, PoisonError, RwLock, TryLockError},
    time::Duration,
};

use async_std::channel::{Receiver, TryRecvError};

// Concerns:
// The sync manager uses channels to talk to the threads
// and the threads use a different set of channels to talk to the sync manager
// I am using bounded channels so that neither party is overwhelmed by the volume of messages
// However, that leads to a deadlock concern where both channels can become full
// leaving each process waiting for the other

#[derive(Clone, Debug)]
pub(crate) struct Backend {
    // pod IP
    ip: String,
    // number of requests outstanding
    reqs: u64,
    // total requests so far
    count: u64,
    // last RTT
    rtt: Duration,
    // weighted avg of RTT --> val := last_val + (cur_val - last_val) / total_so_far
    rtt_mean: Duration,
}

impl Backend {
    pub(crate) fn new(ip: &str) -> Self {
        Self {
            ip: ip.to_owned(),
            reqs: 0,
            count: 0,
            rtt: Duration::default(),
            rtt_mean: Duration::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Service {
    name: String,
    backends: Option<Vec<Backend>>,
}

impl Service {
    pub(crate) fn new(name: &str) -> Self {
        let backends = Self::get_backends(name);
        Self {
            name: name.to_owned(),
            backends,
        }
    }

    pub(crate) fn get_backends(name: &str) -> Option<Vec<Backend>> {
        let b = match reqwest::blocking::get(format!("http://localhost:30000/{}", name)) {
            Ok(resp) => match resp.text() {
                Ok(txt) => txt,
                Err(e) => {
                    println!("Error: {:#?}", e);
                    return None;
                }
            },
            Err(e) => {
                println!("Error: {:#?}", e);
                return None;
            }
        };

        if b.contains("null") {
            return None;
        }

        let mut tmp = b.split(",").collect::<Vec<&str>>();
        tmp.remove(0);

        let tmp0 = match tmp[0].split(":[").nth(1) {
            Some(t) => t,
            None => tmp[0],
        };

        tmp[0] = tmp0;

        // we never expect to hit the `None` branch
        // since the control plane returns each backend in the format
        // "10.16.140.130" and the split gives us the IP in the middle
        let mut ips = tmp
            .drain(..)
            .map(|ip| match ip.split("\"").nth(1) {
                Some(v) => v,
                None => ip,
            })
            .collect::<Vec<&str>>();

        Some(ips.drain(..).map(|ip| Backend::new(ip)).collect())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Mgr {
    receiver: Receiver<Service>, // updates from threads
    services: Arc<RwLock<HashMap<String, Option<Vec<Backend>>>>>, // decomposing service to make search faster
}

impl Mgr {
    pub(crate) fn new(receiver: Receiver<Service>) -> Self {
        Self {
            receiver,
            services: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn get_service(&self, svc: &str) -> Option<Vec<Backend>> {
        match self.services.read() {
            Ok(s) => {
                let key = svc.to_string();
                match s.get(&key) {
                    Some(backends) => backends.clone(),
                    None => Service::get_backends(svc),
                }
            }
            Err(p) => {
                let s = p.get_ref();
                let key = svc.to_string();
                match s.get(&key) {
                    Some(backends) => backends.clone(),
                    None => Service::get_backends(svc),
                }
            }
        }
    }

    pub(crate) fn run(&self) {
        loop {
            match self.receiver.try_recv() {
                Ok(svc) => match self.services.try_write() {
                    Ok(mut s) => {
                        if let Some(service) = s.get_mut(&svc.name) {
                            *service = svc.backends;
                        }
                    }
                    Err(e) => match e {
                        TryLockError::Poisoned(mut p) => {
                            let s = p.get_mut();
                            if let Some(service) = s.get_mut(&svc.name) {
                                *service = svc.backends;
                            }
                        }
                        TryLockError::WouldBlock => continue,
                    },
                },
                Err(e) => match e {
                    TryRecvError::Empty => {}
                    TryRecvError::Closed => {
                        eprintln!("connection closed");
                        break;
                    }
                },
            }
        }
    }
}
