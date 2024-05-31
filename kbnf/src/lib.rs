pub mod config;
pub mod engine_base;
pub mod grammar;
pub mod utils;
pub mod vocabulary;
pub mod engine_like;
pub mod engine;
mod non_zero;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
