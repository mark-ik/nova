// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    ecmascript::types::OrdinaryObject,
    heap::{CompactionLists, HeapMarkAndSweep, WorkQueues},
};

#[derive(Debug, Clone)]
pub(crate) struct EmbedderObjectHeapData<'a> {
    pub(crate) backing_object: Option<OrdinaryObject<'a>>,
    /// Embedder-provided native data. For Genet this carries a `NodeId` (or a
    /// handle into the embedder's side-table) — enough to bridge a JS reflector
    /// back to the host DOM arena. A plain integer: nothing to trace, so the GC
    /// mark/sweep ignore it.
    pub(crate) embedder_data: u64,
    /// Immutable embedder-defined owner identity; preserved by heap snapshots.
    pub(crate) embedder_owner: u64,
}

impl HeapMarkAndSweep for EmbedderObjectHeapData<'static> {
    fn mark_values(&self, queues: &mut WorkQueues) {
        let Self {
            backing_object,
            embedder_data: _,
            embedder_owner: _,
        } = self;
        backing_object.mark_values(queues);
    }

    fn sweep_values(&mut self, compactions: &CompactionLists) {
        let Self {
            backing_object,
            embedder_data: _,
            embedder_owner: _,
        } = self;
        backing_object.sweep_values(compactions);
    }
}
