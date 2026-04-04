use quickbloom::{BloomConfig, ScalableBloomFilter};
use std::path::Path;

fn main() {
    let path = Path::new("example_filter.bin");

    println!("Creating a new Scaleable Bloom Filter...");

    let config = BloomConfig::new(100, 0.01);
    let mut filter = ScalableBloomFilter::new(config).with_persistence(path);

    let items = vec!["alice", "bob", "charlie"];

    for item in &items {
        println!("Inserting '{}'", item);
        filter.insert(item);
    }

    // Explicitly saving it (optional, it also auto-saves on drop!)
    filter.save().expect("Failed to save bloom filter state");
    println!("Saved {} items to example_filter.bin", filter.len());

    // Checking existence
    println!("Checking if 'alice' exists: {}", filter.contains(&"alice"));
    println!("Checking if 'dave' exists: {}", filter.contains(&"dave"));

    if path.exists() {
        std::fs::remove_file(path).unwrap();
    }
}
