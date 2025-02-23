use std::io;
use std::sync::Arc;

use once_cell::sync::Lazy;
use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    lookup_ip::LookupIpIntoIter,
    system_conf,
};

use crate::error::BoxError;

#[derive(Clone)]
pub(crate) struct TrustDnsResolver {
    state: Arc<Mutex<State>>,
}

pub(crate) struct SocketAddrs {
    iter: LookupIpIntoIter,
}

enum State {
    Init,
    Ready(SharedResolver),
}

impl TrustDnsResolver {
    pub(crate) fn new() -> io::Result<Self> {
        SYSTEM_CONF.as_ref().map_err(|e| {
            io::Error::new(e.kind(), format!("error reading DNS system conf: {}", e))
        })?;

        // At this stage, we might not have been called in the context of a
        // Tokio Runtime, so we must delay the actual construction of the
        // resolver.
        Ok(TrustDnsResolver {
            state: Arc::new(Mutex::new(State::Init)),
        })
    }
}

// impl Service<hyper_dns::Name> for TrustDnsResolver {
//     type Response = SocketAddrs;
//     type Error = BoxError;

//     fn poll_ready(&mut self, _: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
//         Poll::Ready(Ok(()))
//     }

//     fn call(&mut self, name: hyper_dns::Name) -> Self::Future {
//         let resolver = self.clone();
//         Box::pin(async move {
//             let mut lock = resolver.state.lock();

//             let resolver = match &*lock {
//                 State::Init => {
//                     let resolver = new_resolver();
//                     *lock = State::Ready(resolver.clone());
//                     resolver
//                 }
//                 State::Ready(resolver) => resolver.clone(),
//             };

//             // Don't keep lock once the resolver is constructed, otherwise
//             // only one lookup could be done at a time.
//             drop(lock);

//             let lookup = resolver.lookup_ip(name.as_str());
//             Ok(SocketAddrs {
//                 iter: lookup.into_iter(),
//             })
//         })
//     }
// }

// impl Iterator for SocketAddrs {
//     type Item = SocketAddr;

//     fn next(&mut self) -> Option<Self::Item> {
//         self.iter.next().map(|ip_addr| SocketAddr::new(ip_addr, 0))
//     }
// }

// fn new_resolver() -> Result<SharedResolver, BoxError> {
//     let (config, opts) = SYSTEM_CONF
//         .as_ref()
//         .expect("can't construct TrustDnsResolver if SYSTEM_CONF is error")
//         .clone();
//     let resolver = AsyncResolver::new(config, opts, TokioHandle)?;
//     Ok(Arc::new(resolver))
// }
