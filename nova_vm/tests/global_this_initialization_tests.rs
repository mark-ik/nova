// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `Realm::finish_global_this_initialization` is the second half of realm
//! creation for an embedder whose global `this` can only be built with a usable
//! agent, and it is restricted to the window in which that is sound.

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

/// Within its window, the installed object is what every way of naming the
/// global `this` resolves to, and all of them agree.
#[test]
fn finished_global_this_is_what_every_lookup_resolves_to() {
    let mut agent = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let realm = agent.create_default_realm();
    agent.run_in_realm(&realm, |agent, mut gc| {
        let replacement: Object = OrdinaryObject::create_empty_object(agent, gc.nogc())
            .unbind()
            .into();
        agent
            .current_realm(gc.nogc())
            .unbind()
            .finish_global_this_initialization(agent, replacement, gc.reborrow())
            .expect("the window is open on a fresh realm");

        for source in ["globalThis", "this", "(function () { return this; })()"] {
            let observed = eval(agent, source, gc.reborrow());
            assert_eq!(
                Object::try_from(observed.unbind()).ok(),
                Some(replacement.unbind()),
                "`{source}` did not resolve to the installed global this"
            );
        }
        let coherent = eval(
            agent,
            "globalThis === this && (function () { return this; })() === globalThis",
            gc.reborrow(),
        );
        assert_eq!(coherent, Value::Boolean(true));
    });
    agent.remove_realm(realm);
}

/// The window closes once code has run in the realm, and closes for good after
/// one successful call.
#[test]
fn finishing_global_this_is_refused_outside_its_window() {
    let mut agent = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let realm = agent.create_default_realm();
    agent.run_in_realm(&realm, |agent, mut gc| {
        let _ = eval(agent, "var ran = 1;", gc.reborrow());
        let replacement: Object = OrdinaryObject::create_empty_object(agent, gc.nogc())
            .unbind()
            .into();
        agent
            .current_realm(gc.nogc())
            .unbind()
            .finish_global_this_initialization(agent, replacement, gc.reborrow())
            .expect_err("a script has already run in this realm");
        let unchanged = eval(agent, "globalThis === this", gc.reborrow());
        assert_eq!(unchanged, Value::Boolean(true));
    });
    agent.remove_realm(realm);

    let mut agent = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let realm = agent.create_default_realm();
    agent.run_in_realm(&realm, |agent, mut gc| {
        let first: Object = OrdinaryObject::create_empty_object(agent, gc.nogc())
            .unbind()
            .into();
        let second: Object = OrdinaryObject::create_empty_object(agent, gc.nogc())
            .unbind()
            .into();
        let current = agent.current_realm(gc.nogc()).unbind();
        current
            .finish_global_this_initialization(agent, first, gc.reborrow())
            .expect("the first call is the initialization");
        current
            .finish_global_this_initialization(agent, second, gc.reborrow())
            .expect_err("the initialization only happens once");
        let observed = eval(agent, "this", gc.reborrow());
        assert_eq!(
            Object::try_from(observed.unbind()).ok(),
            Some(first.unbind())
        );
    });
    agent.remove_realm(realm);
}
