// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nova_vm::{
    ecmascript::{
        Agent, AgentOptions, ArgumentsList, Behaviour, BuiltinFunctionArgs, DefaultHostHooks,
        ExceptionType, Function, GcAgent, InternalMethods, JsResult, PropertyDescriptor,
        PropertyKey, String as JsString, Value, create_builtin_function,
    },
    engine::{Bindable, GcScope, Global},
};

fn probe<'gc>(
    agent: &mut Agent,
    _: Value,
    _: ArgumentsList,
    mut gc: GcScope<'gc, '_>,
) -> JsResult<'gc, Value<'gc>> {
    let caller = agent.native_caller_realm(gc.nogc()).expect("caller");
    let caller = Global::new(agent, caller.unbind());
    let source = JsString::from_string(agent, "thrower()".to_owned(), gc.nogc()).unbind();
    assert!(agent.run_script(source, gc.reborrow()).is_err());
    let expected = caller.take(agent).bind(gc.nogc());
    assert_eq!(agent.native_caller_realm(gc.nogc()), Some(expected));
    Ok(Value::Boolean(expected != agent.current_realm(gc.nogc())))
}

fn thrower<'gc>(
    agent: &mut Agent,
    _: Value,
    _: ArgumentsList,
    gc: GcScope<'gc, '_>,
) -> JsResult<'gc, Value<'gc>> {
    Err(agent.throw_exception_with_static_message(
        ExceptionType::Error,
        "nested throw",
        gc.into_nogc(),
    ))
}

fn relay<'gc>(
    agent: &mut Agent,
    _: Value,
    arguments: ArgumentsList,
    gc: GcScope<'gc, '_>,
) -> JsResult<'gc, Value<'gc>> {
    Function::try_from(arguments.get(0))
        .expect("callback")
        .call(agent, Value::Undefined, &mut [], gc)
}

#[test]
fn native_caller_is_preserved_across_realm_switch_and_nested_throw() {
    let mut agent = GcAgent::new(AgentOptions::default(), &DefaultHostHooks);
    let parent = agent.create_default_realm();
    let child = agent.create_default_realm();
    let function = agent.run_in_realm(&child, |agent, mut gc| {
        for (name, callback) in [
            ("probe", probe as nova_vm::ecmascript::RegularFn),
            ("thrower", thrower as nova_vm::ecmascript::RegularFn),
            ("relay", relay as nova_vm::ecmascript::RegularFn),
        ] {
            let function = create_builtin_function(
                agent,
                Behaviour::Regular(callback),
                BuiltinFunctionArgs::new(0, name),
                gc.nogc(),
            );
            let value = Value::from(function).unbind();
            let global = agent.current_realm(gc.nogc()).global_object(agent).unbind();
            let key = PropertyKey::from_str(agent, name, gc.nogc()).unbind();
            global
                .internal_define_own_property(
                    agent,
                    key,
                    PropertyDescriptor {
                        value: Some(value),
                        ..Default::default()
                    },
                    gc.reborrow(),
                )
                .unwrap();
        }
        let source = JsString::from_string(
            agent,
            "({probe:probe,relay:relay,childCallback:function(){return probe.call(null);}})"
                .to_owned(),
            gc.nogc(),
        )
        .unbind();
        let exports = agent.run_script(source, gc.reborrow()).unwrap().unbind();
        Global::new(agent, exports)
    });
    agent.run_in_realm(&parent, |agent, mut gc| {
        assert!(agent.native_caller_realm(gc.nogc()).is_none());
        let global = agent.current_realm(gc.nogc()).global_object(agent).unbind();
        let key = PropertyKey::from_str(agent, "foreign", gc.nogc()).unbind();
        let value = function.take(agent);
        global
            .internal_define_own_property(
                agent,
                key,
                PropertyDescriptor {
                    value: Some(value),
                    ..Default::default()
                },
                gc.reborrow(),
            )
            .unwrap();
        for (source, expected) in [
            ("foreign.probe()", true),
            ("foreign.probe.call(null)", true),
            ("foreign.probe.apply(null, [])", true),
            ("foreign.relay(foreign.childCallback)", false),
            (
                "foreign.relay(function(){return foreign.probe.apply(null,[]);})",
                true,
            ),
        ] {
            let source = JsString::from_string(agent, source.to_owned(), gc.nogc()).unbind();
            assert_eq!(
                agent.run_script(source, gc.reborrow()).unwrap(),
                Value::Boolean(expected)
            );
            assert!(agent.native_caller_realm(gc.nogc()).is_none());
        }
    });
}
