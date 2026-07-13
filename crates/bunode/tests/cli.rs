#![cfg(unix)]

use std::{
  fs,
  io::{Read, Write},
  net::TcpListener,
  path::{Path, PathBuf},
  process::Command,
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha512};

/// A managed prefix can complete its full patch and revert lifecycle.
///
/// The registry fixture exceeds 10 MiB, matching the size class of real Bun npm packages.
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
  write_executable(&bunode_home.join("node"), "#!/bin/sh\nprintf 'v22.0.0+bun.1.2.3\\n'\n");
  let archive = bun_archive();
  let (registry, server) = serve_registry(archive);

  // 2. Patch through the public CLI and inspect its persisted, runnable result.
  let patched = bunode(&bunode_home)
    .env("BUNODE_REGISTRY", &registry)
    .args(["patch", "1.2.3"])
    .arg(&prefix)
    .arg("--yes")
    .output()
    .unwrap();
  assert_success(&patched);
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

fn bunode(home: &Path) -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_bunode"));
  command.env("BUNODE_HOME", home);
  command
}

fn bun_archive() -> Vec<u8> {
  let script = b"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '1.2.3\\n'; else printf 'v22.0.0\\n'; fi\n";
  let encoder = GzEncoder::new(Vec::new(), Compression::default());
  let mut archive = tar::Builder::new(encoder);
  let mut header = tar::Header::new_gnu();
  header.set_size(script.len() as u64);
  header.set_mode(0o755);
  header.set_cksum();
  archive.append_data(&mut header, "package/bin/bun", &script[..]).unwrap();

  let mut archive = archive.into_inner().unwrap().finish().unwrap();
  archive.resize(10 * 1024 * 1024 + 1, 0);
  archive
}

fn serve_registry(archive: Vec<u8>) -> (String, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let address = listener.local_addr().unwrap();
  let registry = format!("http://{address}");
  let metadata = format!(
    "{{\"dist\":{{\"integrity\":\"sha512-{}\",\"tarball\":\"{registry}/tarball.tgz\"}}}}",
    STANDARD.encode(Sha512::digest(&archive))
  );
  let server = thread::spawn(move || {
    for stream in listener.incoming().take(2) {
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
