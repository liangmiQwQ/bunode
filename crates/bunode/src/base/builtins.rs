//! Node builtin module detection used by preload translation.

const BUILTIN_MODULES: &[&str] = &[
  "_http_agent",
  "_http_client",
  "_http_common",
  "_http_incoming",
  "_http_outgoing",
  "_http_server",
  "_tls_common",
  "_tls_wrap",
  "assert",
  "assert/strict",
  "async_hooks",
  "buffer",
  "child_process",
  "cluster",
  "console",
  "constants",
  "crypto",
  "dgram",
  "diagnostics_channel",
  "dns",
  "dns/promises",
  "domain",
  "events",
  "fs",
  "fs/promises",
  "http",
  "http2",
  "https",
  "inspector",
  "inspector/promises",
  "module",
  "net",
  "node:sea",
  "node:sqlite",
  "node:test",
  "node:test/reporters",
  "os",
  "path",
  "path/posix",
  "path/win32",
  "perf_hooks",
  "process",
  "punycode",
  "querystring",
  "readline",
  "readline/promises",
  "repl",
  "stream",
  "stream/consumers",
  "stream/promises",
  "stream/web",
  "string_decoder",
  "sys",
  "timers",
  "timers/promises",
  "tls",
  "trace_events",
  "tty",
  "url",
  "util",
  "util/types",
  "v8",
  "vm",
  "wasi",
  "worker_threads",
  "zlib",
];

pub(super) fn is_builtin_module(specifier: &str) -> bool {
  let bare_specifier = specifier.strip_prefix("node:").unwrap_or(specifier);

  BUILTIN_MODULES.contains(&specifier) || BUILTIN_MODULES.contains(&bare_specifier)
}

#[cfg(test)]
mod tests {
  use super::is_builtin_module;

  #[test]
  fn should_detect_bare_and_node_prefixed_builtins() {
    assert!(is_builtin_module("fs"));
    assert!(is_builtin_module("fs/promises"));
    assert!(is_builtin_module("node:fs"));
    assert!(is_builtin_module("node:test"));
    assert!(!is_builtin_module("./fs.js"));
  }
}
