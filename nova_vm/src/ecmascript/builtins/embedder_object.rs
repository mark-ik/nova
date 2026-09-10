// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

mod data;

pub(crate) use data::*;

use crate::{
    ecmascript::{
        InternalMethods, InternalSlots, Object, OrdinaryObject, ProtoIntrinsics, WeakKey, WeakRef,
        WeakRefHeapData,
        execution::{Agent, add_to_kept_objects, clear_kept_objects},
        object_handle,
    },
    engine::Bindable,
    heap::{
        ArenaAccess, ArenaAccessMut, BaseIndex, CompactionLists, CreateHeapData, HeapMarkAndSweep,
        HeapSweepWeakReference, WorkQueues, arena_vec_access,
    },
};

/// Embedder objects let an embedder create JS objects carrying native data. Each
/// is backed by an ordinary object (created lazily for property storage) while the
/// embedder owns the native data — for Genet, a `NodeId` bridging the JS reflector
/// back to the host DOM arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EmbedderObject<'a>(BaseIndex<'a, EmbedderObjectHeapData<'static>>);
object_handle!(EmbedderObject);
arena_vec_access!(EmbedderObject, 'a, EmbedderObjectHeapData, embedder_objects);

impl<'a> EmbedderObject<'a> {
    /// Create an embedder object carrying `embedder_data` (e.g. a Genet `NodeId`),
    /// with no backing object yet (created lazily on first property access).
    pub fn create_with_data(agent: &mut Agent, embedder_data: u64) -> EmbedderObject<'static> {
        Self::create_with_owner(agent, embedder_data, 0)
    }

    /// Create native data with an immutable embedder-defined owner identity.
    /// Owner zero preserves the legacy `create_with_data` behavior.
    pub fn create_with_owner(
        agent: &mut Agent,
        embedder_data: u64,
        embedder_owner: u64,
    ) -> EmbedderObject<'static> {
        agent.heap.embedder_objects.push(EmbedderObjectHeapData {
            backing_object: None,
            embedder_data,
            embedder_owner,
        });
        EmbedderObject(BaseIndex::last(&agent.heap.embedder_objects))
    }

    /// Read the immutable owner, independently of script-visible properties.
    pub fn embedder_owner(self, agent: &Agent) -> u64 {
        self.get(agent).embedder_owner
    }

    /// Read back the embedder-provided native data.
    pub fn embedder_data(self, agent: &Agent) -> u64 {
        self.get(agent).embedder_data
    }

    /// Create a [`WeakRef`] weakly targeting this embedder object, for host-side
    /// liveness tracking (the Genet reflector cache). The returned `WeakRef`
    /// must itself be rooted (e.g. in a `Global`); its target — this embedder
    /// object — is held *weakly*, so it can be collected once nothing else
    /// references it. This is the native/embedder entry point; the JS `WeakRef`
    /// constructor is for script. Mirrors the constructor's `AddToKeptObjects`,
    /// so the target survives until the next [`clear_weak_ref_kept_objects`].
    pub fn into_weak_ref(self, agent: &mut Agent) -> WeakRef<'static> {
        let target = WeakKey::EmbedderObject(self.unbind());
        let weak_ref = agent.heap.create(WeakRefHeapData::default());
        weak_ref.set_target(agent, target);
        add_to_kept_objects(agent, target);
        weak_ref
    }

    /// Dereference a [`WeakRef`] created by [`into_weak_ref`](Self::into_weak_ref):
    /// the still-live embedder object, or `None` if it has been collected. Like
    /// the JS `WeakRef.prototype.deref`, observing a live target keeps it alive
    /// until the next [`clear_weak_ref_kept_objects`].
    pub fn from_weak_ref(agent: &mut Agent, weak_ref: WeakRef) -> Option<EmbedderObject<'static>> {
        match weak_ref.get_target(agent) {
            Some(WeakKey::EmbedderObject(eo)) => {
                add_to_kept_objects(agent, WeakKey::EmbedderObject(eo));
                Some(eo.unbind())
            }
            _ => None,
        }
    }
}

/// Clear the WeakRef "kept alive" set (the spec's `ClearKeptObjects`, 9.10).
///
/// The embedder calls this when a synchronous execution sequence completes (the
/// microtask checkpoint), so embedder objects only observed through a
/// [`WeakRef`](EmbedderObject::from_weak_ref) since the last call become
/// collectable again. Without it, every dereferenced reflector would be pinned
/// for the engine's lifetime.
pub fn clear_weak_ref_kept_objects(agent: &mut Agent) {
    clear_kept_objects(agent);
}

impl<'a> InternalSlots<'a> for EmbedderObject<'a> {
    const DEFAULT_PROTOTYPE: ProtoIntrinsics = ProtoIntrinsics::Object;

    #[inline(always)]
    fn get_backing_object(self, agent: &Agent) -> Option<OrdinaryObject<'static>> {
        self.get(agent).backing_object.unbind()
    }

    fn set_backing_object(self, agent: &mut Agent, backing_object: OrdinaryObject<'static>) {
        assert!(
            self.get_mut(agent)
                .backing_object
                .replace(backing_object.unbind())
                .is_none()
        );
    }

    fn create_backing_object(self, agent: &mut Agent) -> OrdinaryObject<'static> {
        let prototype = self.internal_prototype(agent).unwrap();
        let backing_object = OrdinaryObject::create_object(agent, Some(prototype), &[])
            .expect("Should perform GC here")
            .unbind();
        self.set_backing_object(agent, backing_object);
        backing_object
    }

    fn internal_prototype(self, agent: &Agent) -> Option<Object<'static>> {
        if let Some(backing_object) = self.get_backing_object(agent) {
            backing_object.internal_prototype(agent)
        } else {
            Some(
                agent
                    .current_realm_record()
                    .intrinsics()
                    .get_intrinsic_default_proto(ProtoIntrinsics::Object),
            )
        }
    }
}

impl<'a> InternalMethods<'a> for EmbedderObject<'a> {}

impl HeapMarkAndSweep for EmbedderObject<'static> {
    fn mark_values(&self, queues: &mut WorkQueues) {
        queues.embedder_objects.push(*self);
    }

    fn sweep_values(&mut self, compactions: &CompactionLists) {
        compactions.embedder_objects.shift_index(&mut self.0);
    }
}

impl HeapSweepWeakReference for EmbedderObject<'static> {
    fn sweep_weak_reference(self, compactions: &CompactionLists) -> Option<Self> {
        compactions
            .embedder_objects
            .shift_weak_index(self.0)
            .map(Self)
    }
}
