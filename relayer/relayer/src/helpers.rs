pub fn concat_u8_arrays(arrays: Vec<&[u8]>) -> Vec<u8> {
    let mut result = Vec::new();
    for array in arrays {
        result.extend_from_slice(array);
    }
    result
}

pub fn left_pad(coll: Vec<u8>, n: usize) -> Vec<u8> {
    let len = coll.len();
    if len >= n {
        return coll;
    }
    let padding_len = 32 - len;
    let mut padded = vec![0; padding_len];
    padded.extend(coll);
    padded
}
