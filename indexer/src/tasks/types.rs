// async traits?
// https://blog.rust-lang.org/inside-rust/2023/05/03/stabilizing-async-fn-in-trait.html

pub trait TaskBase {
    fn run(&self);
    // fn sleep(&self);
    // fn save(&self);
}
