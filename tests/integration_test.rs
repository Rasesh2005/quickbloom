use quickbloom::BloomFilter;
use std::path::Path;

#[test]
fn test_integration_persistence() {
    let test_file = Path::new("test_filter.bin");
    if test_file.exists() {
        std::fs::remove_file(test_file).unwrap();
    }

    {
        // Notice we chain `with_persistence(path)` now instead of auto-creation.
        let mut filter = BloomFilter::new(5000, 5).with_persistence(test_file);
        filter.insert(&"integration_test_user");
        filter.save().unwrap();
    }

    {
        // Load the filter back explicitly using a path
        let filter = BloomFilter::load_or_new(test_file, 5000, 5);
        assert!(filter.contains(&"integration_test_user"));
        assert!(!filter.contains(&"other_user"));
    }

    if test_file.exists() {
        std::fs::remove_file(test_file).unwrap();
    }
}
