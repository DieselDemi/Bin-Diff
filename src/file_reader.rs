use std::fs::read; 

pub fn read_bin(src: &str) -> Vec<u8> {
    let mut ret = Vec::new(); 

    match read(src) { 
        Ok(bytes) => {
            for byte in bytes { 
                ret.push(byte); 
            }
        }
        Err(e) => { 
            if e.kind() == std::io::ErrorKind::PermissionDenied { 
                eprintln!("Insufficient permissions");  
                return Vec::new(); 
            }

            if e.kind() == std::io::ErrorKind::NotFound { 
                eprintln!("File not found!"); 
                return Vec::new();
            }
            panic!("{}", e); 
        }
    }   
    
    return ret; 
}