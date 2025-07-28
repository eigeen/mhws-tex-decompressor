use indicatif::HumanBytes;

pub fn human_bytes(bytes: u64) -> String {
    HumanBytes(bytes).to_string()
}
