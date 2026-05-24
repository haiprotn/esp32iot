// build.rs — ESP-IDF integration
// embuild tự động tìm và link ESP-IDF toolchain

fn main() {
    // Bắt buộc phải có cho esp-idf-sys
    embuild::espidf::sysenv::output();
}
