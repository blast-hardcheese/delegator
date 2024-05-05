delegator
===

Analysis, Planning, and Routing of cryptograms, a part of the [delegator](https://delegator.dev/) architecture.

## What

The delegator service is an evaluation engine, offered as a key component of the delegator architecture.

Some high level features available via config file without extending the runtime itself:

- Virtualhost-style routing
- [jq](https://jqlang.github.io/jq/)-like JSON transformations, available before and after every step in execution (see [json-adapter](https://github.com/blast-hardcheese/json-adapter) for more)
- Interoperability with existing JSON APIs as steps during evaluation
- Experiment with novel service compositions without redeployment

## Why

The delegator is currently offered in one of two variants, both as a standalone edge layer, as well as an in-progress realization of a broader ecosystem.

## How

For exploratory integrations, it should be sufficient to launch the delegator pointing at a config file.

The configuration files make use of a concept called "cryptogram", as specified in [.../cryptogram.rs](./src/core/model/cryptogram.rs), where `Language` is a jq-inspired transformation language from [json-adapter](https://github.com/blast-hardcheese/json-adapter).

An example config is as follows: <details><summary>example.conf</summary>
<p>

```toml
[events.user_action]  # Stub
queue_url = "noop"

[http]
host = "0.0.0.0"
port = 8080
cors = []

[http.client]
user-agent = "delegator/0.1.0"
default-timeout = "30s"

[services.second] #  #  #  #  #  # Define a service, "second"...
protocol = "rest"
scheme = "http"
authority = "localhost:8080"  #  # Reachable at localhost:8080...

[services.second.methods.world]  # ... with a method "world".
path = "/world"
method = "POST"

[virtualhosts]

[virtualhosts.first]    # Define a virtualhost "first",
hostname = "localhost"  # ... listening on localhost,

[virtualhosts.first.routes."/hello"]  # ... exposing /hello,
                        # running the following program:
cryptogram = """
  {
    "steps": [
      {"payload": "Hello"},
      {"service": "second", "method": "world"}
    ]
  }
  """

[virtualhosts.second]   # Define a virtualhost "second",
hostname = "localhost"  # ... also listening on localhost

[virtualhosts.second.routes."/world"]  # exposing the method we described earlier
                        # running the following program:
cryptogram = """
  {
    "steps": [
      {"preflight": "[., const(\\"World!\\")] | join(\\", \\")"}
    ]
  }
  """
```

</p></details>

When run...

```bash
[nix-shell:~/Projects/delegator]$ cargo run ./helloworld.conf
    Finished dev [unoptimized + debuginfo] target(s) in 0.08s
     Running `target/debug/delegator ./helloworld.conf`
Preparing to bind to 0.0.0.0:8080
[2024-05-05T03:36:13Z INFO  actix_server::builder] starting 14 workers
[2024-05-05T03:36:13Z INFO  actix_server::server] Actix runtime found; starting in Actix runtime
```

... and then hit with cURL:

```bash
$ curl localhost:8080/hello --json '{}'
```

... you'll see (in the access logs,) /world accessed by `delegator/0.1.0`, as well as /hello accessed by cURL:

```
[2024-05-05T03:36:15Z INFO  accesslog] 127.0.0.1 "POST /world HTTP/1.1" 200 15 "-" "delegator/0.1.0" 0.027343
[2024-05-05T03:36:15Z INFO  accesslog] 127.0.0.1 "POST /hello HTTP/1.1" 200 15 "-" "curl/8.4.0" 0.067537
```

... since `/hello` issued a call to `{"service":"second", "method":"world"}`, served by `localhost:8080/world`.

```bash
$ cargo run -- ./config/development.conf
```
