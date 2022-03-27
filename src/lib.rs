mod ctype_wrapper;

use std::stream::from_iter;
use std::sync::Arc;
use std::sync::Mutex;
use std::process::Command;
use std::sync::MutexGuard;

use libc::c_void;
use libc::c_uchar;

use ufo_ipc::*;

type SharedController = Arc<Mutex<ControllerProcess>>;

#[repr(C)]
pub struct Sandbox {
    ptr: *mut c_void,
}
opaque_c_type!(Sandbox, SharedController);

impl Sandbox {
    fn lock(&self) -> MutexGuard<ControllerProcess> {
        let arc = self.deref().unwrap();                     
        let guard = arc.lock().expect("Cannot lock sandbox");
        guard
    }

    #[no_mangle]
    pub extern "C" fn sandbox_start() -> Self {
        std::panic::catch_unwind(|| {
            let child = Command::new("R")
                .args(&["--vanilla", "--no-restore", "-e", "uforemote::start()"])
                .start_subordinate_process()
                .expect("Cannot start R sandbox process: cargo run --bin child"); 
            Self::wrap(Arc::new(Mutex::new(child)))            
        })
        .unwrap_or_else(|_| Self::none())
    }

    #[no_mangle]
    pub extern "C" fn sandbox_shutdown(self, len: usize, data: *const u8) -> () {
        std::panic::catch_unwind(|| {           
            // Prepare data
            let data = CArrayFactory::new(data, len).into();
            
            // Send request
            self.lock().shutdown(&[data])
                .expect("Error while shutting down sandbox")

        })
        .unwrap_or(())
    }

    #[no_mangle]
    pub extern "C" fn sandbox_register_function(&mut self, data_token: u64, serialized_len: usize, serialized_function: *const u8) -> u64 {
        std::panic::catch_unwind(|| {
            // Prepare data
            let data_token = GenericValue::Vu64(data_token);
            let serialized_slice = CArrayFactory::new(serialized_function, serialized_len).into();

            // Send request
            let function_token = 
                self.lock().define_function(serialized_slice, &[data_token], &[])
                    .expect("Cannot register function in sandbox");

            // Unpack response (sends back function token)
            function_token.value.into()

        })
        .unwrap_or(0) // 0 is not a valid function
    }

    #[no_mangle]
    pub extern "C" fn sandbox_register_user_data(&mut self, serialized_len: usize, serialized_data: *const u8) -> u64 {
        std::panic::catch_unwind(|| {
            // Prepare data
            let serialized_slice = CArrayFactory::new(serialized_data, serialized_len).into();

            // Send request
            let data_token = 
                self.lock().poke("new", &[serialized_slice], &[])
                    .expect("Cannot register user data in sandbox");

            // Unpack response (sends back data token)
            let token = data_token.response_aux.first().as_ref()
                .expect("Cannot register user data in sandbox: request did not return an id")              
                .expect_u64()
                .expect("Cannot register user data in sandbox: request returned invalid id");
            *token

        })
        .unwrap_or(0) // 0 is not a valid data token
    }

    pub fn sandbox_call_populate(self, function_token: u64, start: usize, end: usize, data: *mut libc::c_uchar) {
        std::panic::catch_unwind(|| {
            // Prepare data
            let token = FunctionToken::from(function_token);
            let length = end - start;
            let start = GenericValue::Vusize(start);
            let end = GenericValue::Vusize(end);
            let task = GenericValue::Vstring("pop");

            // Send request
            let result = self.lock().call_function(&token, &[start, end], &[task])
                .expect("Cannot call function in sandbox");

            // Unpack response (sends back a page of data)
            let new_data = result.value.first().as_ref()
                .expect("Cannot call function in sandbox: request did not return data")
                .expect_bytes()
                .expect("Cannot call function in sandbox: request did not return a byte array");
            
            // Copy the result into memory
            unsafe {
                let mut memory = core::slice::from_raw_parts_mut(data, length);
                memory.copy_from_slice(new_data.as_slice());
            }

        })
        .unwrap_or(())
    }
}

struct CArrayFactory { data: *const u8, len: usize }
impl CArrayFactory {
    pub fn new(data: *const u8, len: usize) -> Self {
        CArrayFactory { data, len }
    }
}

impl From<CArrayFactory> for &[u8] {
    fn from(factory: CArrayFactory) -> Self {
        unsafe { std::slice::from_raw_parts(factory.data, factory.len) }
    }
}

impl<T> From<CArrayFactory> for GenericValue<&[u8],T> {
    fn from(factory: CArrayFactory) -> Self {
        let array: &[u8] = factory.into();
        GenericValue::Vbytes(array)
    }
}

