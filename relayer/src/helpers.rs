pub fn concat_u8_arrays(arrays: Vec<&[u8]>) -> Vec<u8> {
    let mut result = Vec::new();
    for array in arrays {
        result.extend_from_slice(array);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunks() {
        let from = 0;
        let to = 21;
        let step = 5;
        let intervals = chunks(from, to, step);
        assert_eq!(
            vec![(0, 5), (5, 10), (10, 15), (15, 20), (20, 21)],
            intervals
        );
    }
}
