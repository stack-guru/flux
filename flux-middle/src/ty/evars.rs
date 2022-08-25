use std::{
    hash::BuildHasherDefault,
    sync::{Arc, LazyLock},
};

use dashmap::{lock::RwLock, DashMap};
use flux_common::index::IndexVec;
use rustc_hash::{FxHashMap, FxHasher};
use rustc_index::newtype_index;

use super::{Name, Sort};

type EvarCtxtMap = DashMap<CtxtId, Arc<RwLock<EvarCtxtData>>, BuildHasherDefault<FxHasher>>;

static STORE: LazyLock<EvarCtxtStore> =
    LazyLock::new(|| EvarCtxtStore { map: EvarCtxtMap::default() });

pub struct EvarCtxt {
    arc: Arc<RwLock<EvarCtxtData>>,
}

pub struct EvarCtxtStore {
    map: EvarCtxtMap,
}

struct EvarCtxtData {
    scope: FxHashMap<Name, Sort>,
    evars: IndexVec<EVid, Sort>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EVar {
    cx: CtxtId,
    pub id: EVid,
}

newtype_index! {
    pub struct EVid {
        DEBUG_FORMAT = "?e{}"
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct CtxtId(u64);

impl EvarCtxt {
    pub fn new(scope: impl IntoIterator<Item = (Name, Sort)>) -> EvarCtxt {
        let arc = Arc::new(RwLock::new(EvarCtxtData {
            evars: IndexVec::new(),
            scope: scope.into_iter().collect(),
        }));
        STORE.map.insert(CtxtId::from_arc(&arc), Arc::clone(&arc));
        EvarCtxt { arc }
    }

    pub fn fresh(&self, sort: &Sort) -> EVar {
        let mut data = self.arc.write();
        EVar { cx: CtxtId::from_arc(&self.arc), id: data.evars.push(sort.clone()) }
    }
}

impl CtxtId {
    fn from_arc(arc: &Arc<RwLock<EvarCtxtData>>) -> CtxtId {
        CtxtId(Arc::as_ptr(arc) as u64)
    }
}

impl Drop for EvarCtxt {
    fn drop(&mut self) {
        // When the last `Ref` is dropped, remove the context from the global map.
        if Arc::strong_count(&self.arc) == 2 {
            STORE.map.remove(&CtxtId::from_arc(&self.arc));
        }
    }
}

mod pretty {
    use std::fmt;

    use super::*;
    use crate::pretty::*;

    impl Pretty for EVar {
        fn fmt(&self, _cx: &PPrintCx, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            define_scoped!(cx, f);
            w!("{:?}", ^self.id)
        }
    }
}
