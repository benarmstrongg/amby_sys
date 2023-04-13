pub const RETRY_LIMIT: i32 = 3;

pub fn retry<T>(times: i32, func: impl Fn() -> Option<T>) -> Option<T> {
    let mut retry_count = 0;
    loop {
        if retry_count >= times {
            break;
        }
        let res = match func() {
            Some(res) => Some(res),
            None => {
                retry_count += 1;
                continue;
            }
        };
        return res;
    }
    None
}
