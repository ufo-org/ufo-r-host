mod ctype_wrapper;

use std::sync::Arc;
use std::sync::Mutex, 
use std::process::Command;

use libc::c_void;

use ufo_ipc::*;

type SharedController = Arc<Mutex<ControllerProcess>>;

#[repr(C)]
pub struct CSharedController {
    ptr: *mut c_void,
}
opaque_c_type!(CSharedController, SharedController);

impl CSharedController {
    #[no_mangle]
    pub extern "C" fn shared_controller_start() -> Self {
        std::panic::catch_unwind(|| {
            let child = Command::new("cargo")
                .args(&["run", "--bin", "child"])
                .start_subordinate_process()
                .expect("Cannot start process: cargo run --bin child");    
            
            Self::wrap(Arc::new(Mutex::new(child)))
        })
        .unwrap_or_else(|_| Self::none())
    }

    #[no_mangle]
    pub extern "C" fn shared_controller_shutdown(self, len: usize, data: *const u8) -> () {
        std::panic::catch_unwind(|| {
            let arc = self.deref().unwrap();                     
            let mut guard = arc.lock().expect("Cannot lock shared controller");
        
            let data_slice = unsafe { std::slice::from_raw_parts(data, len) };
            let aux = GenericValue::Vbytes(data_slice);
            guard.shutdown(&[aux]).expect("Error while shutting down shared controller");

        })
        .unwrap_or(())
    }
}