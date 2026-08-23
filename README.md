# Early Cascade Injector

*"The struggle itself toward the heights is enough to fill a man’s heart. One must imagine Sisyphus happy."*  
— Albert Camus

---

## Overview

**Early Cascade Injector** is a proof‑of‑concept Windows x64 process injection technique that leverages the **Shim Engine** inside `ntdll.dll` to execute shellcode **before** the main thread of the target process starts. Combined with **parent process spoofing**, it evades common detection mechanisms by hiding the true origin of the spawned process and by hijacking an early system callback.

The tool is written in **Rust** for reliability and cross‑compilation ease, and includes a custom x86‑64 assembly stub that handles the actual injection and payload execution.

---

## Features

- **Parent process spoofing** – creates the target process (e.g., `Notepad.exe`) as a child of a legitimate process (e.g., `explorer.exe`) to bypass process‑tree based detections.
- **Early injection** – overwrites the Shim Engine’s `se_dll_loaded` and `shims_enabled` pointers in `ntdll` to redirect control flow to the stub before any application code runs.
- **Dynamic pattern scanning** – finds the required offsets in `ntdll` at runtime, making the tool resilient to Windows updates.
- **Obfuscated stub** – the injector’s stub uses junk code and string hashing to hinder static analysis.
- **Encrypted shellcode** – the final payload is XOR‑encrypted and decrypted during injection.
- **Targeted EDR pre‑emption** – the stub enumerates loaded DLLs and can disrupt unknown (likely EDR) modules.

---

## How It Works

1. **Process Creation with Spoofed Parent**  
   The tool uses `CreateProcess` with the `EXTENDED_STARTUPINFO_PRESENT` flag and sets the `PROC_THREAD_ATTRIBUTE_PARENT_PROCESS` attribute to a handle of `explorer.exe`. This makes the new process appear as a child of Explorer.

2. **Suspended State**  
   The target process is created in a suspended state (`CREATE_SUSPENDED`), giving us time to inject code before it starts executing.

3. **Memory Allocation**  
   Two regions are allocated inside the target process:
   - The **stub** (the assembly code that will be executed early).
   - The **encrypted shellcode** (the final payload).

4. **Offset Discovery in ntdll**  
   Using pattern scanning, the tool locates two critical addresses inside `ntdll`:
   - **`se_dll_loaded`** – a pointer that the Shim Engine uses to store the address of the loaded Shim DLL; overwriting it with the stub’s address redirects execution.
   - **`shims_enabled`** – a flag that enables or disables the Shim Engine; setting it to `1` triggers the engine to call our stub.

5. **Patching**  
   - The stub is written to the remote process, with its placeholder (8 `0x11` bytes) replaced by the address of `shims_enabled`.  
   - The shellcode is XOR‑decrypted and written to the remote process.  
   - `se_dll_loaded` is overwritten with the encoded address of the stub.
   - `shims_enabled` is set to `1`.

6. **Execution**  
   Memory protection is changed to `PAGE_EXECUTE_READ`, and the suspended thread is resumed. The Shim Engine then calls the stub, which in turn resolves the `NtQueueApcThread` API from `ntdll` and uses it to execute the decrypted shellcode.

---

## File Structure

```bash
├── src/
│ ├── main.rs # Entry point – injection logic
│ ├── core_file.rs # Pattern scanning, process enumeration, pointer encoding
│ ├── stubs.rs # Binary blobs for the stub and shellcode
│ └── stub/
│ └── stub.asm # Assembly source for the injector stub
```


---

## Building

You need a Rust toolchain (nightly or stable) and the `windows_sys` crate dependency.

1. Clone the repository.
2. Run the build command:

```bash
cargo build
```

## Usage

Simply run the compiled executable. By default it will:

- Look for explorer.exe to obtain its PID.
- Spawn Notepad.exe as a child of Explorer.
- Inject and execute the payload.

Note: The injected shellcode is the one defined in stubs.rs. You can replace it with your own payload (encrypted with the same XOR key "test" or adjust the key accordingly).


## Disclaimer

This project is intended solely for educational and research purposes. It demonstrates advanced Windows injection techniques and should never be used on systems without explicit permission.

## Credits

- [Smukx](https://github.com/Whitecat18/earlycascade-injection)
- [Inspired by research on early cascade injection by outfalnk.](https://www.outflank.nl/blog/2024/10/15/introducing-early-cascade-injection-from-windows-process-creation-to-stealthy-injection/?_gl=1*zoymq3*_up*MQ..*_ga*NjUwNzcyMTc4LjE3ODc1MjI1MzE.*_ga_NHMHGJWX49*czE3ODc1MjI1MzEkbzEkZzAkdDE3ODc1MjI1MzEkajYwJGwwJGgw*_ga_FHB5NMN3M1*czE3ODc1MjI1MzEkbzEkZzAkdDE3ODc1MjI1MzEkajYwJGwwJGgw)