// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `Realm::set_global_this_value` replaces a realm's global `this` after the
//! realm exists, which is what an embedder needs when the replacement can only
//! be built with a usable agent.

use nova_vm::{
    ecmascript::{
        Agent, AgentOptions, DefaultHostHooks, GcAgent, Object, OrdinaryObject, String as JsString,
        Value,
    },
    engine::{Bindable, GcScope},
};

fn eval<'gc>(agent: &mut Agent, source: &str, mut gc: GcScope<'gc, '_>) -> Value<'gc> {
    let source = JsString::from_string(agent, source.to_owned(), gc.nogc()).unbind();
    agent
        .run_script(source, gc.reborrow())
        .expect("script threw")
        .unbind()
        .bind(gc.into_nogc())
}

#[test]
fn set_global_this_value_replaces_the_realm_global_this() {
    let mut agent = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let realm = agent.create_default_realm();
    agent.run_in_realm(&realm, |agent, mut gc| {
        let replacement: Object = OrdinaryObject::create_empty_object(agent, gc.nogc())
            .unbind()
            .into();

        agent
            .current_realm(gc.nogc())
            .unbind()
            .set_global_this_value(agent, replacement, gc.reborrow())
            .expect("set_global_this_value threw");

        for source in [
            "globalThis",
            "(function () { return this; })()",
            "this",
            "globalThis === this",
        ] {
            let observed = eval(agent, source, gc.reborrow());
            if source == "globalThis === this" {
                assert_eq!(observed, Value::Boolean(true), "`{source}`");
            } else {
                assert_eq!(
                    Object::try_from(observed.unbind()).ok(),
                    Some(replacement.unbind()),
                    "`{source}` did not observe the replaced global this"
                );
            }
        }
    });
    agent.remove_realm(realm);
}
