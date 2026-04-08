window.BENCHMARK_DATA = {
  "lastUpdate": 1775654396658,
  "repoUrl": "https://github.com/Rasesh2005/quickbloom",
  "entries": {
    "Rust Benchmark": [
      {
        "commit": {
          "author": {
            "email": "rasesh.udayshetty@gmail.com",
            "name": "Rasesh Shetty",
            "username": "Rasesh2005"
          },
          "committer": {
            "email": "rasesh.udayshetty@gmail.com",
            "name": "Rasesh Shetty",
            "username": "Rasesh2005"
          },
          "distinct": true,
          "id": "12a2ad201b5b2d248c37928833bd5e430517396b",
          "message": "ci: grant workflow write permissions to push to gh-pages",
          "timestamp": "2026-04-08T18:47:40+05:30",
          "tree_id": "eb297e55e5920e5f462dd1dac2d4892f3d8fdd2e",
          "url": "https://github.com/Rasesh2005/quickbloom/commit/12a2ad201b5b2d248c37928833bd5e430517396b"
        },
        "date": 1775654396316,
        "tool": "cargo",
        "benches": [
          {
            "name": "insert/standard",
            "value": 26,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "insert/blocked",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "insert/atomic_lock_free",
            "value": 22,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "contains/standard",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "contains/blocked",
            "value": 23,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "contains/atomic_lock_free",
            "value": 21,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "concurrent/atomic_threads/1",
            "value": 51873,
            "range": "± 1234",
            "unit": "ns/iter"
          },
          {
            "name": "concurrent/atomic_threads/4",
            "value": 124198,
            "range": "± 2962",
            "unit": "ns/iter"
          },
          {
            "name": "concurrent/atomic_threads/8",
            "value": 276659,
            "range": "± 7272",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}