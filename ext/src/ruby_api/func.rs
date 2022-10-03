use super::{
    convert::{ToRubyValue, ToWasmVal},
    func_type::FuncType,
    params::Params,
    root,
    store::{Store, StoreData},
};
use crate::error;
use magnus::{
    block::Proc, function, gc, method, value::BoxValue, DataTypeFunctions, Error, Exception,
    Module as _, Object, RArray, TryConvert, TypedData, Value, QNIL,
};
use wasmtime::{AsContextMut, Caller, Extern, Func as FuncImpl, Trap, Val};

#[derive(TypedData, Debug)]
#[magnus(class = "Wasmtime::Func", mark, size)]
pub struct Func {
    store: Value,
    proc: Value,
    inner: FuncImpl,
}

impl DataTypeFunctions for Func {
    fn mark(&self) {
        gc::mark(&self.store);
        gc::mark(&self.proc);
    }
}

// Wraps a Proc to satisfy wasmtime::Func's Send+Sync requirements. This is safe
// to do as long as (1) we hold the GVL when whe execute the proc and (2) we do
// not have multiple threads running at once (e.g. with Wasm thread proposal).
#[repr(transparent)]
struct ShareableProc(magnus::block::Proc);
unsafe impl Send for ShareableProc {}
unsafe impl Sync for ShareableProc {}

unsafe impl Send for Func {}

impl Func {
    pub fn new(s: Value, functype: &FuncType, _caller: bool, proc: Proc) -> Result<Self, Error> {
        // TODOs:
        // - √ Deal with functype (params and args)
        // - √ Deal with GC. Gotta make sure the proc never gets deleted while we have a reference to it.
        //    - Userland code may not hold a ref to the Func, so can't be the only place we store this.
        // - √ Handle exceptions. Idea: return a wasmtime::TrapReason::Error that
        //   wraps the Ruby exception?  Should we raise that error directly to the
        //   consumer, or should it be a Trap exception with a trap `cause?
        // - Inject the caller (always? or depending on _caller? Would work nicely as a kwarg).

        let store: &Store = s.try_convert()?;
        store.retain(proc.into());
        let mut store = store.borrow_mut();
        let context = store.as_context_mut();
        let ty = functype.get();

        let inner = wasmtime::Func::new(context, ty.clone(), make_func_callable(ty, proc));

        Ok(Self {
            store: s,
            inner,
            proc: proc.into(),
        })
    }

    pub fn get(&self) -> FuncImpl {
        // Makes a copy (wasmtime::Func implements Copy)
        self.inner
    }

    pub fn call(&self, args: RArray) -> Result<Value, Error> {
        let store: &Store = self.store.try_convert()?;
        Self::invoke(store, &self.inner, args).map_err(|e| e.into())
    }

    pub fn invoke(
        store: &Store,
        func: &wasmtime::Func,
        args: RArray,
    ) -> Result<Value, InvokeError> {
        let mut store = store.borrow_mut();
        let func_ty = func.ty(store.as_context_mut());
        let param_types = func_ty.params().collect::<Vec<_>>();
        let params_slice = unsafe { args.as_slice() };
        let params = Params::new(params_slice, param_types)?.to_vec()?;
        let mut results = vec![Val::null(); func_ty.results().len()];

        func.call(store.as_context_mut(), &params, &mut results)
            .map_err(|e| {
                store
                    .as_context_mut()
                    .data_mut()
                    .exception()
                    .take()
                    .map(Error::from)
                    .unwrap_or_else(|| error!("Could not invoke function: {}", e))
            })?;

        match results.as_slice() {
            [] => Ok(QNIL.into()),
            [result] => result.to_ruby_value().map_err(|e| e.into()),
            _ => {
                let array = RArray::with_capacity(results.len());
                for result in results {
                    array.push(result.to_ruby_value()?)?;
                }
                Ok(array.into())
            }
        }
    }
}

impl From<&Func> for Extern {
    fn from(func: &Func) -> Self {
        Self::Func(func.get())
    }
}

fn make_func_callable(
    ty: &wasmtime::FuncType,
    proc: Proc,
) -> impl Fn(Caller<'_, StoreData>, &[Val], &mut [Val]) -> Result<(), Trap> + Send + Sync + 'static
{
    let ty = ty.to_owned();
    let shareable_proc = ShareableProc(proc);

    move |mut caller: Caller<'_, StoreData>, params: &[Val], results: &mut [Val]| {
        let rparams = RArray::with_capacity(params.len());
        for (i, param) in params.iter().enumerate() {
            let rparam = param.to_ruby_value().map_err(|e| {
                wasmtime::Trap::new(format!("invalid argument at index {}: {}", i, e))
            })?;
            rparams.push(rparam).ok();
        }
        let proc = shareable_proc.0;

        proc.call::<RArray, Value>(rparams)
            .map_err(|e| {
                if let Error::Exception(exception) = e {
                    caller.data_mut().exception().hold(exception);
                }
                e
            })
            .and_then(|proc_result| {
                match results.len() {
                    0 => Ok(()), // Ignore return value
                    n => {
                        // For len=1, accept both `val` and `[val]`
                        let proc_result = RArray::try_convert(proc_result)?;
                        if proc_result.len() != n {
                            return Result::Err(error!(
                                "wrong number of results (given {}, expected {})",
                                proc_result.len(),
                                n
                            ));
                        }
                        for ((rb_val, wasm_val), ty) in unsafe { proc_result.as_slice() }
                            .iter()
                            .zip(results.iter_mut())
                            .zip(ty.results())
                        {
                            *wasm_val = rb_val.to_wasm_val(&ty)?;
                        }
                        Ok(())
                    }
                }
            })
            .map_err(|e| {
                wasmtime::Trap::new(format!(
                    "Error when calling Func {}\n Error: {}",
                    proc.inspect(),
                    e
                ))
            })
    }
}

pub enum InvokeError {
    BoxedException(BoxValue<Exception>),
    Error(Error),
}

impl From<InvokeError> for magnus::Error {
    fn from(e: InvokeError) -> Self {
        match e {
            InvokeError::Error(e) => e,
            InvokeError::BoxedException(e) => Error::from(e.to_owned()),
        }
    }
}

impl From<magnus::Error> for InvokeError {
    fn from(e: magnus::Error) -> Self {
        InvokeError::Error(e)
    }
}

impl From<BoxValue<Exception>> for InvokeError {
    fn from(e: BoxValue<Exception>) -> Self {
        InvokeError::BoxedException(e)
    }
}

pub fn init() -> Result<(), Error> {
    let class = root().define_class("Func", Default::default())?;
    class.define_singleton_method("new", function!(Func::new, 4))?;
    class.define_method("call", method!(Func::call, 1))?;

    Ok(())
}
