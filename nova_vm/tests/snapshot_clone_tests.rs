// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::rc::Rc;

use nova_vm::{
    ecmascript::{
        AgentOptions, DefaultHostHooks, GcAgent, RealmRoot, String as JsString, Value,
        parse_script, script_evaluation,
    },
    engine::Bindable,
};

fn eval_i64(agent: &mut GcAgent, realm: &RealmRoot, source: &str) -> i64 {
    agent.run_in_realm(realm, |agent, mut gc| {
        let realm = agent.current_realm(gc.nogc());
        let source_text = JsString::from_string(agent, source.to_owned(), gc.nogc());
        let script = parse_script(agent, source_text, realm, false, None, gc.nogc())
            .unwrap_or_else(|errors| panic!("parse failed for {source:?}: {errors:?}"));
        let value = match script_evaluation(agent, script.unbind(), gc.reborrow()) {
            Ok(value) => value,
            Err(err) => {
                let message_value = err.value().unbind().string_repr(agent, gc);
                let message = message_value.to_string_lossy(agent).into_owned();
                panic!("evaluation failed for {source:?}: {message:?}");
            }
        };
        match value {
            Value::Integer(value) => value.into_i64(),
            other => panic!("expected integer result for {source:?}, got {other:?}"),
        }
    })
}

#[test]
fn snapshot_clone_preserves_js_heap_and_isolates_mutation() {
    let mut base = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let realm = base.create_default_realm();

    assert_eq!(
        eval_i64(
            &mut base,
            &realm,
            "var counter = 1; function bump() { counter += 1; return counter; } counter",
        ),
        1
    );

    let mut clone = base.snapshot_clone(&DefaultHostHooks);

    assert_eq!(eval_i64(&mut clone, &realm, "bump()"), 2);
    assert_eq!(eval_i64(&mut clone, &realm, "counter"), 2);
    assert_eq!(eval_i64(&mut base, &realm, "counter"), 1);
    assert_eq!(eval_i64(&mut base, &realm, "bump()"), 2);
    assert_eq!(eval_i64(&mut clone, &realm, "counter"), 2);
}

#[test]
fn snapshot_clone_deep_clones_array_buffer_storage() {
    let mut base = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let realm = base.create_default_realm();

    assert_eq!(
        eval_i64(
            &mut base,
            &realm,
            "var buf = new ArrayBuffer(4); var view = new Uint8Array(buf); view[0] = 7; view[0]",
        ),
        7
    );

    let mut clone = base.snapshot_clone(&DefaultHostHooks);

    assert_eq!(eval_i64(&mut clone, &realm, "view[0] = 8; view[0]"), 8);
    assert_eq!(eval_i64(&mut base, &realm, "view[0]"), 7);
}

#[test]
fn snapshot_clone_clears_and_replaces_realm_host_defined() {
    let mut base = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let realm = base.create_default_realm();
    realm.initialize_host_defined(&mut base, Rc::new(std::string::String::from("base")));

    let mut clone = base.snapshot_clone(&DefaultHostHooks);
    assert!(
        realm
            .replace_host_defined(
                &mut clone,
                Some(Rc::new(std::string::String::from("clone"))),
            )
            .is_none()
    );

    let original_slot = realm.replace_host_defined(&mut base, None).unwrap();
    let clone_slot = realm.replace_host_defined(&mut clone, None).unwrap();

    assert_eq!(
        original_slot
            .as_ref()
            .downcast_ref::<std::string::String>()
            .unwrap(),
        "base"
    );
    assert_eq!(
        clone_slot
            .as_ref()
            .downcast_ref::<std::string::String>()
            .unwrap(),
        "clone"
    );
}
