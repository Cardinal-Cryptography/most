pub fn concat_u8_arrays(arrays: Vec<&[u8]>) -> Vec<u8> {
    let mut result = Vec::new();
    for array in arrays {
        result.extend_from_slice(array);
    }
    result
}
