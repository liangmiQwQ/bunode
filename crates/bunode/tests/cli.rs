#![cfg(unix)]

use std::{
  fs,
  io::{Read, Write},
  net::TcpListener,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha512};

/// A managed prefix can complete its full patch and revert lifecycle.
///
/// The original prefix's npm fetches a registry fixture larger than 10 MiB.
///
/// Spec: rfcs/bunode-cli.md
#[test]
fn cli_should_patch_list_and_revert_a_prefix() {
  // 1. Set up a Node prefix, wrapper template, and local npm registry.
  let root = temporary_directory();
  let bunode_home = root.join("home");
  let prefix = root.join("node-prefix");
  fs::create_dir_all(prefix.join("bin")).unwrap();
  fs::create_dir_all(&bunode_home).unwrap();
  write_executable(&prefix.join("bin/node"), "#!/bin/sh\nprintf 'v22.0.0\\n'\n");
  write_executable(
    &prefix.join("bin/npm"),
    "#!/bin/sh\nprintf 'used\\n' > \"$BUNODE_TEST_NPM_MARKER\"\nPATH=\"$BUNODE_TEST_PATH\" exec npm \"$@\"\n",
  );
  write_executable(&bunode_home.join("node"), "#!/bin/sh\nprintf 'v22.0.0+bun.1.2.3\\n'\n");
  let npm_cache = root.join("npm-cache");
  let npm_marker = root.join("npm-used");
  fs::create_dir(&npm_cache).unwrap();
  let archive = bun_archive();
  let (registry, server) = serve_registry(archive);

  // 2. Patch through the public CLI and inspect its persisted, runnable result.
  let patched = bunode(&bunode_home)
    .env("BUNODE_REGISTRY", &registry)
    .env("BUNODE_TEST_PATH", std::env::var_os("PATH").unwrap_or_default())
    .env("BUNODE_TEST_NPM_MARKER", &npm_marker)
    .env("npm_config_cache", npm_cache)
    .env("npm_config_fetch_retries", "0")
    .env("npm_config_fetch_timeout", "1000")
    .args(["patch", "1.2.3"])
    .arg(&prefix)
    .arg("--yes")
    .output()
    .unwrap();
  assert_success(&patched);
  assert!(npm_marker.is_file());
  assert!(prefix.join("bun/node.old").is_file());
  assert!(prefix.join("bun/bun").is_file());

  let listed = bunode(&bunode_home).arg("list").output().unwrap();
  assert_success(&listed);
  let list_output = String::from_utf8_lossy(&listed.stdout);
  assert!(list_output.contains("v22.0.0+bun.1.2.3"));
  assert!(list_output.contains("1.2.3"));
  assert!(list_output.contains(prefix.to_string_lossy().as_ref()));

  // 3. Revert through the public CLI and verify the original runtime is restored.
  let reverted = bunode(&bunode_home).arg("revert").arg(&prefix).arg("--yes").output().unwrap();
  assert_success(&reverted);
  let restored = Command::new(prefix.join("bin/node")).output().unwrap();
  assert_eq!(String::from_utf8_lossy(&restored.stdout), "v22.0.0\n");
  assert!(!prefix.join("bun").exists());

  server.join().unwrap();
  fs::remove_dir_all(root).unwrap();
}

/// Declining an in-place version-manager change creates a sibling Bunode prefix.
///
/// Reproduces: choosing No previously exited with `operation cancelled`.
#[test]
fn cli_should_copy_version_managed_prefix_when_in_place_change_is_declined() {
  // 1. Set up a version-managed Node prefix and local npm registry.
  let root = temporary_directory();
  let bunode_home = root.join("home");
  let prefix = root.join("js_runtime/node/22.0.0");
  let target = root.join("js_runtime/node/24.3.0+bun.1.2.3");
  fs::create_dir_all(prefix.join("bin")).unwrap();
  fs::create_dir_all(&bunode_home).unwrap();
  write_executable(&prefix.join("bin/node"), "#!/bin/sh\nprintf 'v22.0.0\\n'\n");
  write_executable(
    &prefix.join("bin/npm"),
    "#!/bin/sh\nPATH=\"$BUNODE_TEST_PATH\" exec npm \"$@\"\n",
  );
  write_executable(&bunode_home.join("node"), "#!/bin/sh\nprintf 'v24.3.0+bun.1.2.3\\n'\n");
  let npm_cache = root.join("npm-cache");
  fs::create_dir(&npm_cache).unwrap();
  let (registry, server) = serve_registry(bun_archive_for_node("v24.3.0"));
  let driver = root.join("patch.sh");
  write_executable(
    &driver,
    "#!/bin/sh\nexec \"$BUNODE_TEST_BIN\" patch 1.2.3 \"$BUNODE_TEST_PREFIX\"\n",
  );

  // 2. Run the public CLI in a PTY and decline modifying the original prefix.
  let mut command = script_command(&driver);
  command
    .env("BUNODE_TEST_BIN", env!("CARGO_BIN_EXE_bunode"))
    .env("BUNODE_TEST_PREFIX", &prefix)
    .env("BUNODE_HOME", &bunode_home)
    .env("BUNODE_REGISTRY", &registry)
    .env("BUNODE_TEST_PATH", std::env::var_os("PATH").unwrap_or_default())
    .env("npm_config_cache", npm_cache)
    .env("npm_config_fetch_retries", "0")
    .env("npm_config_fetch_timeout", "1000")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let mut child = command.spawn().unwrap();
  child.stdin.take().unwrap().write_all(b"yn").unwrap();
  let output = child.wait_with_output().unwrap();

  // 3. Verify the original is untouched and the derived copy is managed and reversible.
  assert!(
    target.join("bun/bun").is_file(),
    "copy was not created:\nstdout: {}\nstderr: {}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(!prefix.join("bun").exists());
  let reverted = bunode(&bunode_home).arg("revert").arg(&target).arg("--yes").output().unwrap();
  assert_success(&reverted);

  server.join().unwrap();
  fs::remove_dir_all(root).unwrap();
}

fn bunode(home: &Path) -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_bunode"));
  command.env("BUNODE_HOME", home);
  command
}

fn script_command(driver: &Path) -> Command {
  let mut command = Command::new("script");
  if cfg!(target_os = "linux") {
    command.args(["-q", "-e", "-c"]).arg(driver).arg("/dev/null");
  } else {
    command.args(["-q", "/dev/null"]).arg(driver);
  }
  command
}

fn bun_archive() -> Vec<u8> {
  bun_archive_for_node("v22.0.0")
}

fn bun_archive_for_node(node_version: &str) -> Vec<u8> {
  let script = format!(
    "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '1.2.3\\n'; else printf '{node_version}\\n'; fi\n"
  );
  let encoder = GzEncoder::new(Vec::new(), Compression::default());
  let mut archive = tar::Builder::new(encoder);
  let mut header = tar::Header::new_gnu();
  header.set_size(script.len() as u64);
  header.set_mode(0o755);
  header.set_cksum();
  archive.append_data(&mut header, "package/bin/bun", script.as_bytes()).unwrap();

  let mut archive = archive.into_inner().unwrap().finish().unwrap();
  archive.resize(10 * 1024 * 1024 + 1, 0);
  archive
}

fn serve_registry(archive: Vec<u8>) -> (String, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let address = listener.local_addr().unwrap();
  let registry = format!("http://{address}");
  let package = platform_package();
  let integrity = format!("sha512-{}", STANDARD.encode(Sha512::digest(&archive)));
  let metadata = format!(
    "{{\"name\":\"{package}\",\"dist-tags\":{{\"latest\":\"1.2.3\"}},\"versions\":{{\"1.2.3\":{{\"name\":\"{package}\",\"version\":\"1.2.3\",\"dist\":{{\"integrity\":\"{integrity}\",\"tarball\":\"{registry}/tarball.tgz\"}}}}}}}}"
  );
  let server = thread::spawn(move || {
    for stream in listener.incoming().take(3) {
      let mut stream = stream.unwrap();
      let mut request = [0_u8; 4096];
      let length = stream.read(&mut request).unwrap();
      let request = String::from_utf8_lossy(&request[..length]);
      let (content_type, body) = if request.contains("GET /tarball.tgz ") {
        ("application/octet-stream", archive.as_slice())
      } else {
        ("application/json", metadata.as_bytes())
      };
      write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
      )
      .unwrap();
      stream.write_all(body).unwrap();
    }
  });

  (registry, server)
}

fn platform_package() -> &'static str {
  match (std::env::consts::OS, std::env::consts::ARCH) {
    ("macos", "aarch64") => "@oven/bun-darwin-aarch64",
    ("macos", "x86_64") => "@oven/bun-darwin-x64",
    ("linux", "aarch64") if cfg!(target_env = "musl") => "@oven/bun-linux-aarch64-musl",
    ("linux", "aarch64") => "@oven/bun-linux-aarch64",
    ("linux", "x86_64") if cfg!(target_env = "musl") => "@oven/bun-linux-x64-musl",
    ("linux", "x86_64") => "@oven/bun-linux-x64",
    (os, arch) => panic!("unsupported test platform {os}-{arch}"),
  }
}

fn write_executable(path: &Path, content: &str) {
  use std::os::unix::fs::PermissionsExt;

  fs::write(path, content).unwrap();
  fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn assert_success(output: &std::process::Output) {
  assert!(
    output.status.success(),
    "command failed:\nstdout: {}\nstderr: {}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

fn temporary_directory() -> PathBuf {
  let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
  let path = std::env::temp_dir().join(format!("bunode-cli-{}-{timestamp}", std::process::id()));
  fs::create_dir(&path).unwrap();
  path
}
