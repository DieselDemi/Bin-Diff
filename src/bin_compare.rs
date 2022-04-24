
///
/// Get the position of a search array if exists.
fn search(bin: &Vec<u8>, search_bytes: &Vec<u8>) -> usize {
    let mut ret = 0;
    for i in 0..bin.len() {
        if bin[i] == search_bytes[0] {
            ret = i;

            for j in 0..search_bytes.len() {
                //Could not find just return 0
                if i + j > bin.len() - 1 {
                    return 0;
                }

                if bin[i + j] != search_bytes[j] {
                    break;
                }
            }
        }
    }

    return ret;
}

/**
 *  Algorithm explanation
 *  1. Take a "string" of bytes and file position from the known mapped bin
 *     a) The string length should be either determined by the user, or by a byte terminator 0x00 (NULL)
 *        for example
 *     b) The string should not be repeating padding like 0xFF 0xFF 0xFF 0xFF 0xFF 0xFF 0xFF 0xFF....
 *  2. Search for that same string in the new bin, if found store that second position
 *  3. Take second position subtract from first position to get offset
 *  4. Push that offset to a list to return.
 */
pub fn compare(chunk_size: usize, known_bin: &Vec<u8>, unknown_bin: &Vec<u8>) -> Result<Vec<(usize, u128)>, &'static str> {
    assert_eq!(known_bin.len(), unknown_bin.len());
    assert_eq!(chunk_size % 2, 0);

    if known_bin.len() != unknown_bin.len() {
        return Err("Length's are not the same");
    }

    // let mut lines: Vec<HashMap<usize, Vec<u8>>> = Vec::new();
    let mut lines: Vec<(usize, Vec<u8>)> = Vec::new();

    let mut line: Vec<u8> = Vec::new();

    let mut ret: Vec<(usize, u128)> = Vec::new();

    for i in 0..known_bin.len() {
        if i % 8 != 0 { continue; }
        if known_bin[i] == 0xff { continue };

        for j in 0..0xf {
            line.push(known_bin[i + j]);
        }

        lines.push((i, line.clone()));
    }

    for line in lines {
        let original_file_offset = line.0;

        let mut chunk_iter: usize = 0;
        for chunk in line.1.chunks(chunk_size) {
            let new_file_offset = search(&unknown_bin, &Vec::from(chunk));
            let chunked_original_file_offset = original_file_offset + chunk_iter * 8;

            if new_file_offset == 0 {
                continue;
            }

            if new_file_offset < chunked_original_file_offset {
                continue;
            }

            ret.push((chunked_original_file_offset, (new_file_offset - chunked_original_file_offset) as u128));

            chunk_iter += 1;
        }
    }

    return Ok(ret);
}