use pyo3::prelude::*;

fn main() {
    pyo3::prepare_freethreaded_python();
    let py_hello = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hello.py"));
    let _ = Python::with_gil(|py| -> Result<(), PyErr> {
        let module = PyModule::from_code(py, py_hello, "", "")?;
        let hello = module.getattr("hello")?;
        let args = (1,4,);
        println!("{}", hello.call(args, None)?);
        Ok(())
    });
}
