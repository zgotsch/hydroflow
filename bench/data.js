window.BENCHMARK_DATA = 
{
  "lastUpdate": 1635553707323,
  "repoUrl": "https://github.com/hydro-project/hydroflow",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "name": "Mingwei Samuel",
            "username": "MingweiSamuel",
            "email": "mingwei.samuel@gmail.com"
          },
          "committer": {
            "name": "Mingwei Samuel",
            "username": "MingweiSamuel",
            "email": "mingwei.samuel@gmail.com"
          },
          "id": "c7a7304291dea4fd8fd95509c17b6aa1d4ea029c",
          "message": "fixup! Update CI for new index.md",
          "timestamp": "2021-10-30T00:20:02Z",
          "url": "https://github.com/hydro-project/hydroflow/commit/c7a7304291dea4fd8fd95509c17b6aa1d4ea029c"
        },
        "date": 1635553707322,
        "tool": "cargo",
        "benches": [
          {
            "name": "arithmetic/hydroflow/compiled",
            "value": 375679,
            "range": "± 2779",
            "unit": "ns/iter"
          },
          {
            "name": "fan_in/hydroflow",
            "value": 203020318,
            "range": "± 942138",
            "unit": "ns/iter"
          },
          {
            "name": "fan_out/hydroflow/scheduled",
            "value": 188451017,
            "range": "± 1640198",
            "unit": "ns/iter"
          },
          {
            "name": "fork_join/hydroflow",
            "value": 8449248,
            "range": "± 44861",
            "unit": "ns/iter"
          },
          {
            "name": "fork_join/hydroflow_builder",
            "value": 41778614,
            "range": "± 190313",
            "unit": "ns/iter"
          },
          {
            "name": "identity/hydroflow",
            "value": 43210330,
            "range": "± 395969",
            "unit": "ns/iter"
          },
          {
            "name": "identity/hydroflow/compiled",
            "value": 12952216,
            "range": "± 8883",
            "unit": "ns/iter"
          },
          {
            "name": "reachability/hydroflow/scheduled",
            "value": 2318677,
            "range": "± 3391",
            "unit": "ns/iter"
          },
          {
            "name": "reachability/hydroflow",
            "value": 2349046,
            "range": "± 2204",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}