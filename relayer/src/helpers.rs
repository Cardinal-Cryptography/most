pub fn concat_u8_arrays(arrays: Vec<&[u8]>) -> Vec<u8> {
    let mut result = Vec::new();
    for array in arrays {
        result.extend_from_slice(array);
    }
    result
}

#[cfg(test)]
mod tests {

    pub fn chunks(from: u32, to: u32, step: u32) -> Vec<(u32, u32)> {
        let mut intervals = Vec::new();
        let mut current = from;

        while current < to {
            let next = current + step;
            if next > to {
                intervals.push((current, to));
            } else {
                intervals.push((current, next));
            }
            current = next;
        }

        intervals
    }

    pub fn pad_zeroes<const A: usize, const B: usize>(arr: [u8; A]) -> [u8; B] {
        assert!(B >= A); //just for a nicer error message, adding #[track_caller] to the function may also be desirable
        let mut b = [0; B];
        b[..A].copy_from_slice(&arr);
        b
    }

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
