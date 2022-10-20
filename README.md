# h2-repro

This reproduces [hyper issue #2419](https://github.com/hyperium/hyper/issues/2419).

`cargo run --release` is enough to show the problem: making `n` POST requests
through `hyper::Client`, where `n` is more than the maximum allowed number of
concurrent streams, results in requests hanging:

```shell
$ cargo run --release --quiet
2022-10-20T20:24:33.711722Z  INFO h2_repro: 100 requests on 50 streams
2022-10-20T20:24:33.711745Z  INFO h2_repro: (Set $H2_REQUESTS and $H2_MAX_STREAMS environment variables to adjust)
2022-10-20T20:24:33.843178Z  INFO h2_repro: H1: Completed 100 / 100
2022-10-20T20:24:34.436008Z ERROR h2_repro: H2: Stuck at 51 / 100
```

A more complete write-up is available at <https://fasterthanli.me/articles/the-http-crash-course-nobody-asked-for#bugs-bugs-bugs>
