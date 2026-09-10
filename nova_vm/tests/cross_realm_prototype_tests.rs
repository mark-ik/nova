// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nova_vm::{
    ecmascript::{
        AgentOptions, DefaultHostHooks, GcAgent, InternalMethods, PropertyDescriptor, PropertyKey,
        RealmRoot, String as JsString, Value,
    },
    engine::{Bindable, Global},
};

fn eval_bool(agent: &mut GcAgent, realm: &RealmRoot, source: &str) {
    agent.run_in_realm(realm, |agent, mut gc| {
        let text = JsString::from_string(agent, source.to_owned(), gc.nogc()).unbind();
        assert_eq!(
            agent.run_script(text, gc.reborrow()).unwrap(),
            Value::Boolean(true),
            "{source}"
        );
    });
}

#[test]
fn builtin_prototypes_keep_their_creation_realm_across_reads_and_gc() {
    let mut agent = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let parent = agent.create_default_realm();
    let child = agent.create_default_realm();
    let global = agent.run_in_realm(&child, |agent, mut gc| {
        // Do not inspect instance prototypes here: the first such read must be
        // in the other realm, before any ordinary backing object is demanded.
        let text = JsString::from_string(
            agent,
            r#"
            globalThis.values = [new Map(), new Set(), new ArrayBuffer(8),
                new Uint8Array(8), new DataView(new ArrayBuffer(8)), new Date(),
                /x/, new Error(), Promise.resolve(), [],
                [][Symbol.iterator](), new Map().entries(), new Set().values(),
                ''[Symbol.iterator](), 'x'.matchAll(/x/g), Object(1), Object('x'),
                Object(true), Object(1n), Object(Symbol()), new Uint8Array(1).buffer,
                (function () { try { null.x; } catch (error) { return error; } })(),
                (function () { try { missingRealmVariable; } catch (error) { return error; } })()];
            globalThis.expected = [Map.prototype, Set.prototype, ArrayBuffer.prototype,
                Uint8Array.prototype, DataView.prototype, Date.prototype,
                RegExp.prototype, Error.prototype, Promise.prototype, Array.prototype,
                Object.getPrototypeOf([][Symbol.iterator]()),
                Object.getPrototypeOf(new Map().entries()),
                Object.getPrototypeOf(new Set().values()),
                Object.getPrototypeOf(''[Symbol.iterator]()),
                Object.getPrototypeOf('x'.matchAll(/x/g)), Number.prototype, String.prototype,
                Boolean.prototype, BigInt.prototype, Symbol.prototype, ArrayBuffer.prototype,
                TypeError.prototype, ReferenceError.prototype];
        "#
            .to_owned(),
            gc.nogc(),
        )
        .unbind();
        agent.run_script(text, gc.reborrow()).unwrap();
        let global = agent.current_realm(gc.nogc()).global_object(agent).unbind();
        Global::new(agent, Value::from(global))
    });
    agent.run_in_realm(&parent, |agent, mut gc| {
        let child = global.take(agent);
        let global = agent.current_realm(gc.nogc()).global_object(agent).unbind();
        let key = PropertyKey::from_str(agent, "child", gc.nogc()).unbind();
        global
            .internal_define_own_property(
                agent,
                key,
                PropertyDescriptor {
                    value: Some(child),
                    writable: Some(true),
                    configurable: Some(true),
                    enumerable: Some(true),
                    ..Default::default()
                },
                gc.reborrow(),
            )
            .unwrap();
    });
    let check = r#"(function () {
        for (var i=0; i<child.values.length; ++i) {
            if (Object.getPrototypeOf(child.values[i]) !== child.expected[i])
                throw new Error('wrong birth prototype at index '+i);
        }
        return true;
    })()"#;
    eval_bool(&mut agent, &parent, check);
    agent.remove_realm(child);
    agent.gc();
    agent.gc();
    eval_bool(&mut agent, &parent, check);
    eval_bool(
        &mut agent,
        &parent,
        r#"(function () {
        function Target() {} Target.prototype = 0;
        var foreignDefault = Reflect.construct(child.Map, [], Target);
        if (Object.getPrototypeOf(foreignDefault) !== Map.prototype) return false;
        Target.prototype = child.Map.prototype;
        var explicit = Reflect.construct(Map, [], Target);
        if (Object.getPrototypeOf(explicit) !== child.Map.prototype) return false;
        var custom = {};
        Object.setPrototypeOf(child.values[0], custom);
        return Object.getPrototypeOf(child.values[0]) === custom;
    })()"#,
    );
}
