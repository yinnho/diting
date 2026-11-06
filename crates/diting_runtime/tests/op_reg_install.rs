//! 认领战役步骤⑤钉测：自有 op 注册面端到端。
//!
//! `#[diting_op]` 生成的 wrapper（宏 + op_glue 全是我们自己的代码）+
//! `OpSet`（自有 OpCtx/OpDecl/state）+ 裸 `Vm` 安装——全程零 deno_core。
//! 刻意不测的两张面（宏的发射还引用 deno_core，宿主换批翻掉后这里补）：
//! state 参数（OpCtx decode 还是 deno 的）与 Result 返回（Err 臂走
//! deno 的 to_v8_error）。同步数值/字符串/解码失败面在这钉死。

use diting_ops::diting_op;
use diting_runtime::op_reg::OpDecl;
use diting_runtime::vm::Vm;

#[diting_op]
fn __t_add(a: u32, b: i32) -> i32 {
    a as i32 + b
}

#[diting_op]
fn __t_f(x: f64) -> f64 {
    x * 2.0
}

#[diting_op]
fn __t_bool(x: bool) -> bool {
    !x
}

#[diting_op]
fn __t_smi(#[smi] n: u32) -> f64 {
    n as f64 + 0.5
}

#[diting_op]
#[string]
fn __t_echo(#[string] s: &str) -> String {
    format!("echo:{s}")
}

#[diting_op]
#[string]
fn __t_own(#[string] s: String) -> String {
    s.to_uppercase()
}

#[diting_op]
#[buffer]
fn __t_rev(#[buffer] b: &[u8]) -> Vec<u8> {
    b.iter().rev().copied().collect()
}

#[diting_op]
fn __t_unit(#[string] s: &str) {
    let _ = s;
}

/// `#[object]` return face (absorbed from serde_v8 on the v8 152 bump).
struct Tile {
    w: usize,
    data: Vec<u8>,
}

impl diting_runtime::op_glue::ToV8Object for Tile {
    fn to_v8_object<'a, 'i>(
        self,
        scope: &mut v8::PinScope<'a, 'i>,
    ) -> Result<v8::Local<'a, v8::Value>, diting_runtime::op_glue::EncodeError> {
        use diting_runtime::op_glue::RustToV8Fallible;
        let obj = v8::Object::new(scope);
        let w = v8::Number::new(scope, self.w as f64);
        let data = self.data.to_v8_fallible(scope)?;
        let k = v8::String::new(scope, "w").unwrap();
        obj.set(scope, k.into(), w.into()).unwrap();
        let k = v8::String::new(scope, "data").unwrap();
        obj.set(scope, k.into(), data).unwrap();
        Ok(obj.into())
    }
}

#[diting_op]
#[object]
fn __t_tile(#[buffer] b: &[u8]) -> Result<Tile, deno_error::JsErrorBox> {
    if b.is_empty() {
        return Err(deno_error::JsErrorBox::type_error("empty tile"));
    }
    Ok(Tile {
        w: b.len(),
        data: b.to_vec(),
    })
}

fn vm_with_ops() -> Vm {
    let mut vm = Vm::new();
    let decls: Vec<OpDecl> = vec![
        __t_add(),
        __t_f(),
        __t_bool(),
        __t_smi(),
        __t_echo(),
        __t_own(),
        __t_rev(),
        __t_unit(),
        __t_tile(),
    ];
    vm.install_ops(decls);
    vm
}

#[test]
fn sync_ops_decode_and_encode_roundtrip() {
    let mut vm = vm_with_ops();
    assert_eq!(vm.eval_string("__t_add(2, 3)").unwrap(), "5");
    // u32 4294967295 wraps to -1 through the `as i32` cast inside the op —
    // the deliberate wrap-on-overflow semantics the glue doc notes.
    assert_eq!(vm.eval_string("__t_add(4294967295, -1)").unwrap(), "-2");
    assert_eq!(vm.eval_string("__t_f(2.25)").unwrap(), "4.5");
    assert_eq!(vm.eval_string("__t_bool(true)").unwrap(), "false");
    assert_eq!(vm.eval_string("__t_smi(7)").unwrap(), "7.5");
    assert_eq!(vm.eval_string("__t_echo('hi')").unwrap(), "echo:hi");
    assert_eq!(vm.eval_string("__t_own('abc')").unwrap(), "ABC");
    assert_eq!(
        vm.eval_string("__t_rev(new Uint8Array([1,2,3])).join(',')")
            .unwrap(),
        "3,2,1"
    );
    assert_eq!(vm.eval_string("__t_unit('x')").unwrap(), "undefined");
}

#[test]
fn fn_length_reflects_arg_count() {
    let mut vm = vm_with_ops();
    // state args don't exist here; every arg is JS-visible.
    assert_eq!(vm.eval_string("__t_add.length").unwrap(), "2");
    assert_eq!(vm.eval_string("__t_echo.length").unwrap(), "1");
}

#[test]
fn decode_failure_throws_type_error_with_message() {
    let mut vm = vm_with_ops();
    let err = vm.eval_string("__t_add('x', 1)").unwrap_err();
    let msg = match &err {
        diting_runtime::vm::LoopError::TickThrew(m) => m.clone(),
        other => panic!("expected TickThrew, got {other:?}"),
    };
    assert!(msg.contains("expected u32"), "message was: {msg}");
    // And the vm survives a thrown decode error — next call is fine.
    assert_eq!(vm.eval_string("__t_add(1, 1)").unwrap(), "2");
}

#[test]
fn constructor_call_throws() {
    let mut vm = vm_with_ops();
    // Installed with ConstructorBehavior::Throw (the learned primitive).
    let err = vm.eval_string("new __t_add(1, 1)").unwrap_err();
    assert!(matches!(err, diting_runtime::vm::LoopError::TickThrew(_)));
}

#[test]
fn ops_survive_poll_rounds() {
    // The OpSet lives in the Vm (Externals point into it); loop passes must
    // not disturb the installed functions.
    let mut vm = vm_with_ops();
    assert_eq!(vm.eval_string("__t_add(20, 22)").unwrap(), "42");
    vm.poll().unwrap();
    assert_eq!(vm.eval_string("__t_add(20, 23)").unwrap(), "43");
}

#[test]
fn buffer_arg_reads_the_view_window_not_the_whole_store() {
    // BufferSlice (absorbed V8Slice): byte_offset/byte_length of the view,
    // a detached store reads as empty, and a non-Uint8Array is a TypeError.
    let mut vm = vm_with_ops();
    assert_eq!(
        vm.eval_string("__t_rev(new Uint8Array([9,1,2,3,9]).subarray(1,4)).join(',')")
            .unwrap(),
        "3,2,1"
    );
    assert_eq!(
        vm.eval_string("const u = new Uint8Array([1,2]); u.buffer.transfer(); __t_rev(u).length")
            .unwrap(),
        "0"
    );
    let err = vm.eval_string("__t_rev([1,2])").unwrap_err();
    let diting_runtime::vm::LoopError::TickThrew(msg) = &err else {
        panic!("expected TickThrew, got {err:?}");
    };
    assert!(
        msg.contains("expected typed ArrayBufferView"),
        "message was: {msg}"
    );
}

#[test]
fn object_return_builds_a_plain_object() {
    let mut vm = vm_with_ops();
    assert_eq!(
        vm.eval_string(
            "const t = __t_tile(new Uint8Array([4,5,6])); \
             [Object.getPrototypeOf(t) === Object.prototype, Object.keys(t).join(), \
              t.w, t.data instanceof Uint8Array, t.data.join()].join('|')"
        )
        .unwrap(),
        "true|w,data|3|true|4,5,6"
    );
    // The Err arm still throws through to_v8_error.
    let err = vm.eval_string("__t_tile(new Uint8Array(0))").unwrap_err();
    let diting_runtime::vm::LoopError::TickThrew(msg) = &err else {
        panic!("expected TickThrew, got {err:?}");
    };
    assert!(msg.contains("empty tile"), "message was: {msg}");
}
